use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(test)]
#[path = "config_test.rs"]
mod config_test;

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: PathBuf,

    #[serde(default)]
    pub auth: AuthConfig,

    #[serde(default)]
    pub channels: ChannelsConfig,

    #[serde(default)]
    pub cron: CronConfig,

    #[serde(default)]
    pub node: NodeConfig,

    // Ported from zeroclaw src/memory/sqlite.rs architecture
    #[serde(default)]
    pub memory: MemoryConfig,

    // Ported from zeroclaw src/tunnel/mod.rs schema pattern
    #[serde(default)]
    pub tunnel: TunnelConfig,

    #[serde(default)]
    pub gateway: GatewayConfig,

    #[serde(default)]
    pub tools: ToolsConfig,
}

fn default_workspace_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/workspace"))
        .unwrap_or_else(|| PathBuf::from(".rustclaw/workspace"))
}

fn default_state_dir() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/state"))
        .unwrap_or_else(|| PathBuf::from(".rustclaw/state"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_model")]
    pub default_model: String,

    // Ported from zeroclaw src/providers/reliable.rs
    #[serde(default)]
    pub reliability: ProviderReliabilityConfig,

    // Ported from zeroclaw src/providers/router.rs
    #[serde(default)]
    pub routes: Vec<ProviderRoute>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            default_provider: default_provider(),
            default_model: default_model(),
            reliability: ProviderReliabilityConfig::default(),
            routes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderReliabilityConfig {
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_backoff_ms")]
    pub base_backoff_ms: u64,
    #[serde(default)]
    pub fallback_order: Vec<String>,
}

impl Default for ProviderReliabilityConfig {
    fn default() -> Self {
        Self {
            max_retries: default_max_retries(),
            base_backoff_ms: default_backoff_ms(),
            fallback_order: vec!["anthropic".into(), "openai".into()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRoute {
    pub hint: String,
    pub provider: String,
    pub model: String,
}

fn default_max_retries() -> u32 { 2 }
fn default_backoff_ms() -> u64 { 600 }
fn default_provider() -> String {
    "anthropic".into()
}
fn default_model() -> String {
    "claude-sonnet-4-20250514".into()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelsConfig {
    pub telegram: Option<TelegramConfig>,
    pub whatsapp: Option<WhatsAppConfig>,
    pub slack: Option<SlackConfig>,
    pub discord: Option<DiscordConfig>,
    pub signal: Option<SignalConfig>,
    #[serde(default)]
    pub health: ChannelHealthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    #[serde(default)]
    pub allow_from: Vec<String>,
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    pub api_url: Option<String>,
    pub access_token: Option<String>,
    pub verify_token: Option<String>,
    pub phone_number_id: Option<String>,
    #[serde(default = "default_wa_port")]
    pub webhook_port: u16,
    pub app_secret: Option<String>,
}

fn default_wa_port() -> u16 {
    8090
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    pub app_token: Option<String>,
    pub signing_secret: Option<String>,
    #[serde(default = "bool_true")]
    pub socket_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    pub bot_token: Option<String>,
    #[serde(default)]
    pub guild_ids: Vec<String>,
    #[serde(default)]
    pub channel_ids: Vec<String>,
    #[serde(default = "default_discord_poll_interval_secs")]
    pub poll_interval_secs: u64,
}

fn default_discord_poll_interval_secs() -> u64 { 3 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    pub number: Option<String>,
    pub api_url: Option<String>,
    #[serde(default = "default_signal_poll_interval_secs")]
    pub poll_interval_secs: u64,
}

fn default_signal_poll_interval_secs() -> u64 { 3 }

fn bool_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronConfig {
    #[serde(default)]
    pub jobs: Vec<CronJob>,
    #[serde(default)]
    pub enable_heartbeat: bool,
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_min: u32,
}

fn default_heartbeat_interval() -> u32 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub task: String,
    pub channel: Option<String>,
    pub target: Option<String>,
    #[serde(default = "bool_true")]
    pub enabled: bool,
    // Ported from zeroclaw cron retry knobs (simplified)
    #[serde(default = "default_max_retries")]
    pub retries: u32,
    /// Timeout in seconds. None or Some(0) = no timeout.
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    /// Per-job webhook URL for delivery notifications.
    #[serde(default)]
    pub delivery_webhook_url: Option<String>,
    /// Stagger window in seconds for anti-thundering-herd. None = auto (0-30s).
    #[serde(default)]
    pub stagger_seconds: Option<u64>,
    /// If true, run at exact scheduled time (no stagger).
    #[serde(default)]
    pub exact: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeConfig {
    pub name: Option<String>,
    #[serde(default = "default_beacon_interval")]
    pub beacon_interval_secs: u64,
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
}

fn default_beacon_interval() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_backend")]
    pub backend: String,
    #[serde(default = "default_memory_db")]
    pub sqlite_path: String,
    #[serde(default = "default_memory_results")]
    pub max_results: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: default_memory_backend(),
            sqlite_path: default_memory_db(),
            max_results: default_memory_results(),
        }
    }
}

fn default_memory_backend() -> String { "sqlite".into() }
fn default_memory_db() -> String { "memory/brain.db".into() }
fn default_memory_results() -> usize { 8 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    #[serde(default = "default_tunnel_provider")]
    pub provider: String,
    pub custom_start_command: Option<String>,
    pub public_url: Option<String>,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            provider: default_tunnel_provider(),
            custom_start_command: None,
            public_url: None,
        }
    }
}

fn default_tunnel_provider() -> String { "none".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    #[serde(default = "default_gateway_host")]
    pub host: String,
    #[serde(default = "default_gateway_port")]
    pub port: u16,
    #[serde(default)]
    pub auth: GatewayAuthConfig,
    #[serde(default)]
    pub rate_limit: GatewayRateLimitConfig,
    #[serde(default)]
    pub cors: GatewayCorsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayAuthConfig {
    #[serde(default = "default_gateway_auth_mode")]
    pub mode: String,
    pub token: Option<String>,
}

impl Default for GatewayAuthConfig {
    fn default() -> Self {
        Self {
            mode: default_gateway_auth_mode(),
            token: None,
        }
    }
}

fn default_gateway_auth_mode() -> String { "none".into() }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayRateLimitConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    #[serde(default = "default_gateway_rate_limit_rpm")]
    pub requests_per_minute: u32,
}

impl Default for GatewayRateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: default_gateway_rate_limit_rpm(),
        }
    }
}

fn default_gateway_rate_limit_rpm() -> u32 { 120 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayCorsConfig {
    #[serde(default)]
    pub allowed_origins: Vec<String>,
}

impl Default for GatewayCorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec!["*".into()],
        }
    }
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_gateway_host(),
            port: default_gateway_port(),
            auth: GatewayAuthConfig::default(),
            rate_limit: GatewayRateLimitConfig::default(),
            cors: GatewayCorsConfig::default(),
        }
    }
}

fn default_gateway_host() -> String { "127.0.0.1".into() }
fn default_gateway_port() -> u16 { 8088 }

// -- Tools config (Feature 1: URL allowlists) --

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolsConfig {
    #[serde(default)]
    pub web: WebConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebConfig {
    /// Allowed domains for web tools. Empty = all allowed.
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

// -- Channel health check config (Feature 10) --

fn default_health_check_minutes() -> u64 { 5 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelHealthConfig {
    #[serde(default = "default_health_check_minutes")]
    pub check_interval_minutes: u64,
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

impl Default for ChannelHealthConfig {
    fn default() -> Self {
        Self {
            check_interval_minutes: default_health_check_minutes(),
            enabled: true,
        }
    }
}

pub fn load_config(path: Option<&Path>) -> Result<Config> {
    let config_path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            directories::BaseDirs::new()
                .map(|d| d.home_dir().join(".rustclaw/config.toml"))
                .unwrap_or_else(|| PathBuf::from("config.toml"))
        });

    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("reading config from {}", config_path.display()))?;
        let cfg: Config = toml::from_str(&content)
            .with_context(|| format!("parsing config from {}", config_path.display()))?;
        Ok(cfg)
    } else {
        tracing::debug!("no config file found at {}, using defaults", config_path.display());
        Ok(Config {
            workspace_dir: default_workspace_dir(),
            auth: AuthConfig::default(),
            channels: ChannelsConfig::default(),
            cron: CronConfig::default(),
            node: NodeConfig::default(),
            memory: MemoryConfig::default(),
            tunnel: TunnelConfig::default(),
            gateway: GatewayConfig::default(),
            tools: ToolsConfig::default(),
        })
    }
}
