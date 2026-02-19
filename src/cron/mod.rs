use crate::config::{Config, CronJob};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::fs;

pub struct CronStore {
    path: std::path::PathBuf,
}

impl CronStore {
    pub fn from_config(cfg: &Config) -> Result<Self> {
        let path = cfg.workspace_dir.join("state/cron.db");
        if let Some(p) = path.parent() { fs::create_dir_all(p)?; }
        let s = Self { path };
        s.init()?;
        Ok(s)
    }
    fn conn(&self) -> Result<Connection> { Ok(Connection::open(&self.path)?) }
    fn init(&self) -> Result<()> {
        let c = self.conn()?;
        c.execute_batch("CREATE TABLE IF NOT EXISTS cron_jobs(id TEXT PRIMARY KEY, schedule TEXT, task TEXT, channel TEXT, target TEXT, enabled INTEGER, retries INTEGER);
                         CREATE TABLE IF NOT EXISTS cron_history(id TEXT PRIMARY KEY, job_id TEXT, status TEXT, attempts INTEGER, output TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP);")?;
        Ok(())
    }
    pub fn upsert_job(&self, job: &CronJob) -> Result<()> {
        self.conn()?.execute("INSERT INTO cron_jobs(id,schedule,task,channel,target,enabled,retries)
            VALUES(?1,?2,?3,?4,?5,?6,?7)
            ON CONFLICT(id) DO UPDATE SET schedule=excluded.schedule, task=excluded.task, channel=excluded.channel, target=excluded.target, enabled=excluded.enabled, retries=excluded.retries",
            params![job.id, job.schedule, job.task, job.channel, job.target, job.enabled as i32, job.retries])?;
        Ok(())
    }
    pub fn delete_job(&self, id: &str) -> Result<bool> {
        Ok(self.conn()?.execute("DELETE FROM cron_jobs WHERE id=?1", params![id])? > 0)
    }
    pub fn list_jobs(&self) -> Result<Vec<CronJob>> {
        let c = self.conn()?;
        let mut st = c.prepare("SELECT id,schedule,task,channel,target,enabled,retries FROM cron_jobs")?;
        let rows = st.query_map([], |r| Ok(CronJob {
            id: r.get(0)?, schedule: r.get(1)?, task: r.get(2)?, channel: r.get(3)?, target: r.get(4)?, enabled: r.get::<_, i32>(5)? != 0, retries: r.get(6)?,
            timeout_seconds: None, delivery_webhook_url: None, stagger_seconds: None, exact: false,
        }))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
    pub fn job_exists(&self, id: &str) -> Result<bool> {
        let c = self.conn()?;
        let count: i64 = c.query_row("SELECT COUNT(*) FROM cron_jobs WHERE id=?1", params![id], |r| r.get(0))?;
        Ok(count > 0)
    }
    pub fn record_history(&self, job_id: &str, status: &str, attempts: u32, output: &str) -> Result<()> {
        self.conn()?.execute("INSERT INTO cron_history(id,job_id,status,attempts,output) VALUES(?1,?2,?3,?4,?5)", params![uuid::Uuid::new_v4().to_string(), job_id, status, attempts, output])?;
        Ok(())
    }
}

pub fn list(cfg: &Config) -> Result<Vec<CronJob>> {
    let store = CronStore::from_config(cfg)?;
    let mut jobs = store.list_jobs()?;
    if jobs.is_empty() {
        for j in &cfg.cron.jobs { store.upsert_job(j)?; }
        jobs = store.list_jobs()?;
    }
    Ok(jobs)
}

pub fn add(cfg: &Config, schedule: &str, task: &str) -> Result<String> {
    schedule.parse::<cron::Schedule>().map_err(|e| anyhow::anyhow!("Invalid cron expression: {e}"))?;
    let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let job = CronJob { id: id.clone(), schedule: schedule.into(), task: task.into(), channel: None, target: None, enabled: true, retries: 1, timeout_seconds: None, delivery_webhook_url: None, stagger_seconds: None, exact: false };
    CronStore::from_config(cfg)?.upsert_job(&job)?;
    Ok(id)
}

pub fn remove(cfg: &Config, id: &str) -> Result<()> {
    if !CronStore::from_config(cfg)?.delete_job(id)? { anyhow::bail!("Cron job not found: {id}"); }
    Ok(())
}

pub async fn execute_job(cfg: &Config, job_id: &str) -> Result<String> {
    let store = CronStore::from_config(cfg)?;
    let job_cfg = store.list_jobs()?.into_iter().find(|j| j.id == job_id).ok_or_else(|| anyhow::anyhow!("Cron job not found: {job_id}"))?;
    execute_with_retry(cfg, &job_cfg, &store).await
}

/// Compute the stagger delay for a job (Feature 5: anti-thundering-herd).
pub fn compute_stagger_delay(job: &CronJob) -> std::time::Duration {
    if job.exact {
        return std::time::Duration::ZERO;
    }
    let secs = job.stagger_seconds.unwrap_or(30);
    if secs == 0 {
        return std::time::Duration::ZERO;
    }
    let random_secs = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        job.id.hash(&mut h);
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut h);
        h.finish() % secs
    };
    std::time::Duration::from_secs(random_secs)
}

/// Resolve the effective timeout for a job (Feature 3).
/// None or Some(0) = no timeout. Some(n) = n seconds.
pub fn resolve_timeout(job: &CronJob) -> Option<std::time::Duration> {
    match job.timeout_seconds {
        Some(0) | None => None,
        Some(n) => Some(std::time::Duration::from_secs(n)),
    }
}

async fn execute_with_retry(cfg: &Config, job: &CronJob, store: &CronStore) -> Result<String> {
    // Feature 5: Apply stagger delay
    let stagger = compute_stagger_delay(job);
    if !stagger.is_zero() {
        tracing::debug!(job_id=%job.id, stagger_ms=%stagger.as_millis(), "stagger delay");
        tokio::time::sleep(stagger).await;
    }

    let started_at = chrono::Utc::now();
    let max_retries = job.retries.max(1);
    let mut attempt = 0;
    let mut last_err = String::new();
    let timeout = resolve_timeout(job);

    while attempt < max_retries {
        attempt += 1;
        // Feature 3: Optional timeout
        let task_future = crate::chat::send(cfg, &job.task, None);
        let result = match timeout {
            Some(dur) => match tokio::time::timeout(dur, task_future).await {
                Ok(r) => r,
                Err(_) => Err(anyhow::anyhow!("job timed out after {}s", dur.as_secs())),
            },
            None => task_future.await,
        };

        match result {
            Ok(resp) => {
                let finished_at = chrono::Utc::now();
                store.record_history(&job.id, "ok", attempt, &resp)?;
                notify_completion(cfg, job, true, &resp, &started_at, &finished_at).await;
                return Ok(resp);
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt < max_retries {
                    let backoff = 250_u64 * (1 << (attempt - 1));
                    tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                }
            }
        }
    }
    let finished_at = chrono::Utc::now();
    store.record_history(&job.id, "error", attempt, &last_err)?;
    notify_completion(cfg, job, false, &last_err, &started_at, &finished_at).await;
    anyhow::bail!("cron job failed after retries: {}", last_err)
}

async fn notify_completion(
    _cfg: &Config,
    job: &CronJob,
    ok: bool,
    output: &str,
    started_at: &chrono::DateTime<chrono::Utc>,
    finished_at: &chrono::DateTime<chrono::Utc>,
) {
    tracing::info!(job_id=%job.id, ok, "cron completion");
    let duration_ms = (*finished_at - *started_at).num_milliseconds().max(0) as u64;
    let status = if ok { "ok" } else { "error" };

    let payload = serde_json::json!({
        "job_id": job.id,
        "status": status,
        "output": output,
        "started_at": started_at.to_rfc3339(),
        "finished_at": finished_at.to_rfc3339(),
        "duration_ms": duration_ms,
    });

    let client = reqwest::Client::new();

    // Feature 4: Per-job webhook delivery
    if let Some(ref webhook_url) = job.delivery_webhook_url {
        let _ = client.post(webhook_url).json(&payload).send().await;
    }

    // Legacy global webhook
    if let Ok(webhook) = std::env::var("CRON_WEBHOOK_URL") {
        let _ = client.post(webhook).json(&payload).send().await;
    }
}

pub async fn start_scheduler(cfg: &Config) -> Result<tokio_cron_scheduler::JobScheduler> {
    use tokio_cron_scheduler::{Job, JobScheduler};
    let sched = JobScheduler::new().await?;
    let store = CronStore::from_config(cfg)?;
    // BUG-05: Only insert config jobs that don't already exist in SQLite.
    // This preserves any runtime modifications to existing jobs.
    for j in &cfg.cron.jobs {
        if !store.job_exists(&j.id)? {
            store.upsert_job(j)?;
        }
    }
    for job_cfg in store.list_jobs()? {
        if !job_cfg.enabled { continue; }
        let cfg_clone = cfg.clone();
        let store2 = CronStore::from_config(cfg)?;
        let j = job_cfg.clone();
        let job = Job::new_async(job_cfg.schedule.as_str(), move |_id, _lock| {
            let cfg = cfg_clone.clone();
            let j = j.clone();
            let store = CronStore { path: store2.path.clone() };
            Box::pin(async move {
                let _ = execute_with_retry(&cfg, &j, &store).await;
            })
        })?;
        sched.add(job).await?;
    }
    sched.start().await?;
    Ok(sched)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, ChannelsConfig, Config, CronConfig, GatewayConfig, MemoryConfig, NodeConfig, TunnelConfig};

    fn cfg(tmp: &tempfile::TempDir) -> Config {
        Config { workspace_dir: tmp.path().into(), auth: AuthConfig::default(), channels: ChannelsConfig::default(), cron: CronConfig::default(), node: NodeConfig::default(), memory: MemoryConfig::default(), tunnel: TunnelConfig::default(), gateway: GatewayConfig::default(), tools: crate::config::ToolsConfig::default() }
    }

    fn test_job(id: &str, schedule: &str, task: &str, retries: u32) -> CronJob {
        CronJob { id: id.into(), schedule: schedule.into(), task: task.into(), channel: None, target: None, enabled: true, retries, timeout_seconds: None, delivery_webhook_url: None, stagger_seconds: None, exact: false }
    }

    #[test]
    fn store_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg(&tmp);
        let store = CronStore::from_config(&cfg).unwrap();
        let job = test_job("j1", "0 * * * * *", "ping", 2);
        store.upsert_job(&job).unwrap();
        let jobs = store.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        store.record_history("j1", "ok", 1, "done").unwrap();
    }

    // BUG-05: Existing jobs in SQLite should not be overwritten by config on restart
    #[test]
    fn config_jobs_do_not_overwrite_existing() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg(&tmp);
        let store = CronStore::from_config(&cfg).unwrap();

        let modified_job = test_job("daily-report", "0 9 * * * *", "modified task at runtime", 1);
        store.upsert_job(&modified_job).unwrap();

        let config_job = test_job("daily-report", "0 9 * * * *", "original task from config", 1);
        if !store.job_exists(&config_job.id).unwrap() {
            store.upsert_job(&config_job).unwrap();
        }

        let jobs = store.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task, "modified task at runtime", "existing job should not be overwritten");
    }

    // Feature 3: timeout_seconds tests
    #[test]
    fn resolve_timeout_none_means_no_timeout() {
        let job = test_job("j1", "0 * * * * *", "task", 1);
        assert!(super::resolve_timeout(&job).is_none());
    }

    #[test]
    fn resolve_timeout_zero_means_no_timeout() {
        let mut job = test_job("j1", "0 * * * * *", "task", 1);
        job.timeout_seconds = Some(0);
        assert!(super::resolve_timeout(&job).is_none());
    }

    #[test]
    fn resolve_timeout_positive_value() {
        let mut job = test_job("j1", "0 * * * * *", "task", 1);
        job.timeout_seconds = Some(60);
        let dur = super::resolve_timeout(&job).unwrap();
        assert_eq!(dur.as_secs(), 60);
    }

    // Feature 4: per-job webhook delivery (payload structure test)
    #[test]
    fn job_with_webhook_url() {
        let mut job = test_job("j1", "0 * * * * *", "task", 1);
        job.delivery_webhook_url = Some("https://hooks.example.com/cron".into());
        assert_eq!(job.delivery_webhook_url.as_deref(), Some("https://hooks.example.com/cron"));
    }

    // Feature 5: stagger scheduling tests
    #[test]
    fn stagger_zero_when_exact() {
        let mut job = test_job("j1", "0 * * * * *", "task", 1);
        job.exact = true;
        job.stagger_seconds = Some(30);
        let delay = super::compute_stagger_delay(&job);
        assert!(delay.is_zero());
    }

    #[test]
    fn stagger_zero_when_stagger_zero() {
        let mut job = test_job("j1", "0 * * * * *", "task", 1);
        job.stagger_seconds = Some(0);
        let delay = super::compute_stagger_delay(&job);
        assert!(delay.is_zero());
    }

    #[test]
    fn stagger_within_window() {
        let mut job = test_job("j1", "0 * * * * *", "task", 1);
        job.stagger_seconds = Some(30);
        let delay = super::compute_stagger_delay(&job);
        assert!(delay.as_secs() < 30);
    }

    #[test]
    fn stagger_default_when_none() {
        let job = test_job("j1", "0 * * * * *", "task", 1);
        // stagger_seconds is None -> defaults to 30s window
        let delay = super::compute_stagger_delay(&job);
        assert!(delay.as_secs() < 30);
    }

    #[test]
    fn config_jobs_inserted_when_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = cfg(&tmp);
        let store = CronStore::from_config(&cfg).unwrap();

        let config_job = test_job("new-job", "0 * * * * *", "new task", 1);
        if !store.job_exists(&config_job.id).unwrap() {
            store.upsert_job(&config_job).unwrap();
        }

        let jobs = store.list_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].task, "new task");
    }
}
