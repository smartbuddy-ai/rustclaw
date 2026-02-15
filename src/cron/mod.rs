use crate::config::{Config, CronJob};
use anyhow::Result;
use std::fs;

/// List all cron jobs.
pub fn list(cfg: &Config) -> Result<Vec<CronJob>> {
    Ok(cfg.cron.jobs.clone())
}

/// Add a cron job and persist to config.
pub fn add(cfg: &Config, schedule: &str, task: &str) -> Result<String> {
    // Validate cron expression
    schedule.parse::<cron::Schedule>()
        .map_err(|e| anyhow::anyhow!("Invalid cron expression: {e}"))?;

    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();

    let job = CronJob {
        id: id.clone(),
        schedule: schedule.into(),
        task: task.into(),
        channel: None,
        target: None,
        enabled: true,
    };

    // Append to config file
    let mut config = cfg.clone();
    config.cron.jobs.push(job);
    save_config(&config)?;

    Ok(id)
}

/// Remove a cron job by ID.
pub fn remove(cfg: &Config, id: &str) -> Result<()> {
    let mut config = cfg.clone();
    let before = config.cron.jobs.len();
    config.cron.jobs.retain(|j| j.id != id);
    if config.cron.jobs.len() == before {
        anyhow::bail!("Cron job not found: {id}");
    }
    save_config(&config)?;
    Ok(())
}

fn config_path() -> std::path::PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/config.toml"))
        .unwrap_or_else(|| std::path::PathBuf::from("config.toml"))
}

fn save_config(cfg: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = toml::to_string_pretty(cfg)?;
    fs::write(&path, content)?;
    Ok(())
}

/// Execute a single cron job (for manual testing).
pub async fn execute_job(cfg: &Config, job_id: &str) -> Result<String> {
    let job_cfg = cfg
        .cron
        .jobs
        .iter()
        .find(|j| j.id == job_id)
        .ok_or_else(|| anyhow::anyhow!("Cron job not found: {job_id}"))?;

    tracing::info!(job_id = %job_id, task = %job_cfg.task, "manually executing cron job");
    let response = crate::chat::send(cfg, &job_cfg.task, None).await?;
    Ok(response)
}

/// Start the cron scheduler, executing jobs on schedule.
pub async fn start_scheduler(cfg: &Config) -> Result<tokio_cron_scheduler::JobScheduler> {
    use tokio_cron_scheduler::{Job, JobScheduler};

    let sched = JobScheduler::new().await?;

    // Add automatic heartbeat job if enabled
    if cfg.cron.enable_heartbeat {
        let interval = cfg.cron.heartbeat_interval_min;
        let schedule_str = format!("0 */{interval} * * * *"); // Every N minutes
        let cfg_clone = cfg.clone();

        let heartbeat_job = Job::new_async(&schedule_str, move |_uuid, _lock| {
            let cfg = cfg_clone.clone();
            Box::pin(async move {
                tracing::info!("heartbeat triggered");
                if let Err(e) = execute_heartbeat(&cfg).await {
                    tracing::error!(error = %e, "heartbeat execution failed");
                }
            })
        })?;

        sched.add(heartbeat_job).await?;
        tracing::info!(interval_min = interval, "heartbeat job registered");
    }

    for job_cfg in &cfg.cron.jobs {
        if !job_cfg.enabled {
            continue;
        }

        let task = job_cfg.task.clone();
        let cfg_clone = cfg.clone();
        let job_id = job_cfg.id.clone();
        let schedule_str = job_cfg.schedule.clone();
        let channel = job_cfg.channel.clone();
        let target = job_cfg.target.clone();

        let job = Job::new_async(schedule_str.as_str(), move |_uuid, _lock| {
            let task = task.clone();
            let cfg = cfg_clone.clone();
            let jid = job_id.clone();
            let chan = channel.clone();
            let tgt = target.clone();
            Box::pin(async move {
                tracing::info!(job_id = %jid, task = %task, "cron job triggered");
                match crate::chat::send(&cfg, &task, None).await {
                    Ok(resp) => {
                        tracing::info!(job_id = %jid, response_len = resp.len(), "cron job completed");
                        
                        // If channel and target are specified, send the response there
                        if let (Some(channel), Some(target)) = (chan, tgt) {
                            if let Err(e) = send_cron_output(&cfg, &channel, &target, &resp).await {
                                tracing::error!(error = %e, "failed to send cron output to channel");
                            }
                        }
                    }
                    Err(e) => tracing::error!(job_id = %jid, error = %e, "cron job failed"),
                }
            })
        })?;

        sched.add(job).await?;
        tracing::debug!(id = %job_cfg.id, schedule = %job_cfg.schedule, "registered cron job");
    }

    sched.start().await?;
    Ok(sched)
}

/// Execute heartbeat tasks from HEARTBEAT.md
async fn execute_heartbeat(cfg: &Config) -> Result<()> {
    let heartbeat_content = crate::workspace::read_file(cfg, "HEARTBEAT.md")?;
    
    if let Some(content) = heartbeat_content {
        if content.trim().is_empty() {
            tracing::debug!("HEARTBEAT.md is empty, skipping");
            return Ok(());
        }

        tracing::info!("executing heartbeat tasks");
        let prompt = format!(
            "You are performing your heartbeat check. Read the following HEARTBEAT.md file and execute any tasks it specifies:\n\n{content}\n\nIf there are no tasks or nothing needs attention, respond with 'HEARTBEAT_OK'."
        );

        match crate::chat::send(cfg, &prompt, None).await {
            Ok(resp) => {
                if resp.trim() != "HEARTBEAT_OK" {
                    tracing::info!(response_len = resp.len(), "heartbeat produced output");
                    // Could send this to a channel if configured
                } else {
                    tracing::debug!("heartbeat: all quiet");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "heartbeat execution failed");
            }
        }
    } else {
        tracing::debug!("HEARTBEAT.md not found, skipping");
    }

    Ok(())
}

/// Send cron job output to a channel (Telegram, Slack, etc.)
async fn send_cron_output(
    cfg: &Config,
    channel: &str,
    target: &str,
    message: &str,
) -> Result<()> {
    match channel {
        "telegram" => {
            if let Some(tg_cfg) = &cfg.channels.telegram {
                let token = tg_cfg
                    .bot_token
                    .clone()
                    .or_else(|| std::env::var("TELEGRAM_BOT_TOKEN").ok())
                    .ok_or_else(|| anyhow::anyhow!("TELEGRAM_BOT_TOKEN not set"))?;

                let client = reqwest::Client::new();
                let chat_id: i64 = target.parse()?;

                #[derive(serde::Serialize)]
                struct SendMsg {
                    chat_id: i64,
                    text: String,
                }

                client
                    .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
                    .json(&SendMsg {
                        chat_id,
                        text: message.into(),
                    })
                    .send()
                    .await?;
            }
        }
        _ => {
            tracing::warn!(channel = channel, "unsupported cron output channel");
        }
    }
    Ok(())
}
