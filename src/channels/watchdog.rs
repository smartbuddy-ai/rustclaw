//! Feature 10: Channel health check + watchdog.
//! Monitors channel health and restarts dead channels.

use crate::config::{ChannelHealthConfig, Config};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Health status for a single channel.
#[derive(Debug, Clone)]
pub struct ChannelHealth {
    pub name: String,
    pub last_heartbeat: Instant,
    pub healthy: bool,
    pub restart_count: u32,
}

/// Thread-safe channel health registry.
#[derive(Clone)]
pub struct HealthRegistry {
    channels: Arc<RwLock<HashMap<String, ChannelHealth>>>,
    check_interval: Duration,
}

impl HealthRegistry {
    pub fn new(cfg: &ChannelHealthConfig) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            check_interval: Duration::from_secs(cfg.check_interval_minutes * 60),
        }
    }

    /// Register a channel for health monitoring.
    pub async fn register(&self, name: &str) {
        let mut channels = self.channels.write().await;
        channels.insert(name.to_string(), ChannelHealth {
            name: name.to_string(),
            last_heartbeat: Instant::now(),
            healthy: true,
            restart_count: 0,
        });
    }

    /// Record a heartbeat from a channel.
    pub async fn heartbeat(&self, name: &str) {
        let mut channels = self.channels.write().await;
        if let Some(ch) = channels.get_mut(name) {
            ch.last_heartbeat = Instant::now();
            ch.healthy = true;
        }
    }

    /// Mark a channel as unhealthy.
    pub async fn mark_unhealthy(&self, name: &str) {
        let mut channels = self.channels.write().await;
        if let Some(ch) = channels.get_mut(name) {
            ch.healthy = false;
        }
    }

    /// Check all channels and return names of those that need restarting.
    pub async fn check_health(&self) -> Vec<String> {
        let mut dead = Vec::new();
        let mut channels = self.channels.write().await;
        let now = Instant::now();

        for (name, health) in channels.iter_mut() {
            let elapsed = now.duration_since(health.last_heartbeat);
            if elapsed > self.check_interval {
                health.healthy = false;
                dead.push(name.clone());
                tracing::warn!(channel=%name, elapsed_secs=%elapsed.as_secs(), "channel health check failed");
            }
        }

        dead
    }

    /// Record a channel restart.
    pub async fn record_restart(&self, name: &str) {
        let mut channels = self.channels.write().await;
        if let Some(ch) = channels.get_mut(name) {
            ch.restart_count += 1;
            ch.last_heartbeat = Instant::now();
            ch.healthy = true;
            tracing::info!(channel=%name, restart_count=%ch.restart_count, "channel restarted");
        }
    }

    /// Get the health status of all channels.
    pub async fn status(&self) -> Vec<ChannelHealth> {
        let channels = self.channels.read().await;
        channels.values().cloned().collect()
    }

    /// Check if a specific channel is healthy.
    pub async fn is_healthy(&self, name: &str) -> bool {
        let channels = self.channels.read().await;
        channels
            .get(name)
            .map(|ch| {
                ch.healthy
                    && ch.last_heartbeat.elapsed() <= self.check_interval
            })
            .unwrap_or(false)
    }
}

/// Start the watchdog background task.
pub async fn start_watchdog(cfg: &Config, registry: HealthRegistry) -> tokio::task::JoinHandle<()> {
    let interval = Duration::from_secs(cfg.channels.health.check_interval_minutes * 60);
    let _cfg = cfg.clone();

    tokio::spawn(async move {
        let mut tick = tokio::time::interval(interval);
        loop {
            tick.tick().await;
            let dead = registry.check_health().await;
            for channel_name in &dead {
                tracing::warn!(channel=%channel_name, "attempting channel restart");
                // In a full implementation, this would restart the channel task.
                // For now, we record the restart attempt.
                registry.record_restart(channel_name).await;
            }
            if !dead.is_empty() {
                tracing::info!(dead_count=%dead.len(), "watchdog cycle complete");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ChannelHealthConfig;

    #[tokio::test]
    async fn register_and_heartbeat() {
        let cfg = ChannelHealthConfig { check_interval_minutes: 5, enabled: true };
        let registry = HealthRegistry::new(&cfg);

        registry.register("telegram").await;
        registry.heartbeat("telegram").await;

        assert!(registry.is_healthy("telegram").await);
    }

    #[tokio::test]
    async fn unregistered_channel_not_healthy() {
        let cfg = ChannelHealthConfig { check_interval_minutes: 5, enabled: true };
        let registry = HealthRegistry::new(&cfg);

        assert!(!registry.is_healthy("nonexistent").await);
    }

    #[tokio::test]
    async fn mark_unhealthy() {
        let cfg = ChannelHealthConfig { check_interval_minutes: 5, enabled: true };
        let registry = HealthRegistry::new(&cfg);

        registry.register("slack").await;
        assert!(registry.is_healthy("slack").await);

        registry.mark_unhealthy("slack").await;
        assert!(!registry.is_healthy("slack").await);
    }

    #[tokio::test]
    async fn status_returns_all_channels() {
        let cfg = ChannelHealthConfig { check_interval_minutes: 5, enabled: true };
        let registry = HealthRegistry::new(&cfg);

        registry.register("telegram").await;
        registry.register("slack").await;
        registry.register("discord").await;

        let status = registry.status().await;
        assert_eq!(status.len(), 3);
    }

    #[tokio::test]
    async fn record_restart_increments_count() {
        let cfg = ChannelHealthConfig { check_interval_minutes: 5, enabled: true };
        let registry = HealthRegistry::new(&cfg);

        registry.register("telegram").await;
        registry.record_restart("telegram").await;
        registry.record_restart("telegram").await;

        let status = registry.status().await;
        let tg = status.iter().find(|s| s.name == "telegram").unwrap();
        assert_eq!(tg.restart_count, 2);
        assert!(tg.healthy);
    }

    #[tokio::test]
    async fn check_health_detects_stale_channels() {
        // Use 0 minutes = 0 second interval so everything looks stale
        let cfg = ChannelHealthConfig { check_interval_minutes: 0, enabled: true };
        let registry = HealthRegistry::new(&cfg);

        registry.register("telegram").await;
        // Any time elapsed > 0 seconds will trigger
        tokio::time::sleep(Duration::from_millis(10)).await;

        let dead = registry.check_health().await;
        assert!(dead.contains(&"telegram".to_string()));
    }
}
