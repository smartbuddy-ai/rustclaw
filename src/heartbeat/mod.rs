use crate::config::Config;
use anyhow::Result;

pub struct HeartbeatConfig {
    pub interval_secs: u64,
    pub active_start_hour: u8,
    pub active_end_hour: u8,
    pub prompt: String,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self { interval_secs: 1800, active_start_hour: 8, active_end_hour: 23, prompt: "Read HEARTBEAT.md and act. Reply HEARTBEAT_OK when nothing to do.".into() }
    }
}

pub async fn run(cfg: Config, hb: HeartbeatConfig) -> Result<()> {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(hb.interval_secs));
    loop {
        ticker.tick().await;
        if !is_active_hours(hb.active_start_hour, hb.active_end_hour, chrono::Local::now().hour() as u8) {
            continue;
        }
        let resp = crate::chat::send(&cfg, &hb.prompt, None).await.unwrap_or_else(|e| format!("HEARTBEAT_ERROR: {e}"));
        process_heartbeat_response(&resp);
    }
}

pub fn is_active_hours(start: u8, end: u8, hour: u8) -> bool {
    if start <= end { hour >= start && hour < end } else { hour >= start || hour < end }
}

pub fn process_heartbeat_response(resp: &str) {
    if resp.trim() == "HEARTBEAT_OK" {
        tracing::debug!("heartbeat no-op");
    } else {
        tracing::info!(len = resp.len(), "heartbeat produced actionable output");
    }
}

use chrono::Timelike;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_hours_works() {
        assert!(is_active_hours(8, 23, 10));
        assert!(!is_active_hours(8, 23, 2));
        assert!(is_active_hours(22, 6, 23));
        assert!(is_active_hours(22, 6, 3));
    }

    #[test]
    fn heartbeat_ok_no_panic() {
        process_heartbeat_response("HEARTBEAT_OK");
        process_heartbeat_response("do task");
    }
}
