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

/// Start the cron scheduler, executing jobs on schedule.
pub async fn start_scheduler(cfg: &Config) -> Result<tokio_cron_scheduler::JobScheduler> {
    use tokio_cron_scheduler::{Job, JobScheduler};

    let sched = JobScheduler::new().await?;

    for job_cfg in &cfg.cron.jobs {
        if !job_cfg.enabled {
            continue;
        }

        let task = job_cfg.task.clone();
        let cfg_clone = cfg.clone();
        let job_id = job_cfg.id.clone();
        let schedule_str = job_cfg.schedule.clone();

        let job = Job::new_async(schedule_str.as_str(), move |_uuid, _lock| {
            let task = task.clone();
            let cfg = cfg_clone.clone();
            let jid = job_id.clone();
            Box::pin(async move {
                tracing::info!(job_id = %jid, task = %task, "cron job triggered");
                match crate::chat::send(&cfg, &task, None).await {
                    Ok(resp) => tracing::info!(job_id = %jid, response = %resp, "cron job completed"),
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
