pub mod telegram;
pub mod whatsapp;
pub mod slack;
pub mod discord;
pub mod signal;
pub mod watchdog;

use crate::config::Config;
use serde::{Deserialize, Serialize};

/// Inbound message from any channel.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel: String,
    pub chat_id: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub text: String,
    pub message_id: Option<String>,
    pub reply_to_message_id: Option<String>,
    pub thread_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Outbound message to any channel.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub chat_id: String,
    pub text: String,
    pub reply_to_message_id: Option<String>,
    pub thread_id: Option<String>,
}

/// Channel router: starts all enabled channels concurrently.
pub fn start_enabled_channels(cfg: &Config) -> Vec<tokio::task::JoinHandle<()>> {
    let mut handles = Vec::new();

    if let Some(ref tg) = cfg.channels.telegram {
        if tg.enabled {
            let tg = tg.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = telegram::run(&cfg2, &tg).await {
                    tracing::error!(channel = "telegram", error = %e, "channel exited");
                }
            }));
        }
    }

    if let Some(ref wa) = cfg.channels.whatsapp {
        if wa.enabled {
            let wa = wa.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = whatsapp::run(&cfg2, &wa).await {
                    tracing::error!(channel = "whatsapp", error = %e, "channel exited");
                }
            }));
        }
    }

    if let Some(ref sl) = cfg.channels.slack {
        if sl.enabled {
            let sl = sl.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = slack::run(&cfg2, &sl).await {
                    tracing::error!(channel = "slack", error = %e, "channel exited");
                }
            }));
        }
    }

    if let Some(ref dc) = cfg.channels.discord {
        if dc.enabled {
            let dc = dc.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = discord::run(&cfg2, &dc).await {
                    tracing::error!(channel = "discord", error = %e, "channel exited");
                }
            }));
        }
    }

    if let Some(ref sg) = cfg.channels.signal {
        if sg.enabled {
            let sg = sg.clone();
            let cfg2 = cfg.clone();
            handles.push(tokio::spawn(async move {
                if let Err(e) = signal::run(&cfg2, &sg).await {
                    tracing::error!(channel = "signal", error = %e, "channel exited");
                }
            }));
        }
    }

    handles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_router_starts_none_when_no_channels() {
        let cfg = Config {
            workspace_dir: std::path::PathBuf::from("."),
            auth: crate::config::AuthConfig::default(),
            channels: crate::config::ChannelsConfig::default(),
            cron: crate::config::CronConfig::default(),
            node: crate::config::NodeConfig::default(),
            memory: crate::config::MemoryConfig::default(),
            tunnel: crate::config::TunnelConfig::default(),
            gateway: crate::config::GatewayConfig::default(),
            tools: crate::config::ToolsConfig::default(),
        };
        let handles = start_enabled_channels(&cfg);
        assert!(handles.is_empty());
    }
}
