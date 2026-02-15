use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Top-level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Workspace directory for .md files, memory/, etc.
    #[serde(default = "default_workspace_dir")]
    pub workspace_dir: PathBuf,

    /// LLM auth configuration
    #[serde(default)]
    pub auth: AuthConfig,

    /// Channel configurations
    #[serde(default)]
    pub channels: ChannelsConfig,

    /// Cron jobs
    #[serde(default)]
    pub cron: CronConfig,

    /// Node / presence settings
    #[serde(default)]
    pub node: NodeConfig,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Anthropic API key (or env: ANTHROPIC_API_KEY)
    pub anthropic_api_key: Option<String>,
    /// OpenAI API key (or env: OPENAI_API_KEY)
    pub openai_api_key: Option<String>,
    /// Default model provider: "anthropic" or "openai"
    #[serde(default = "default_provider")]
    pub default_provider: String,
    /// Default model name
    #[serde(default = "default_model")]
    pub default_model: String,
}

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// Bot token (or env: TELEGRAM_BOT_TOKEN)
    pub bot_token: Option<String>,
    /// Allowed user IDs
    #[serde(default)]
    pub allow_from: Vec<String>,
    /// Webhook URL (if empty, use polling)
    pub webhook_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// WhatsApp Business API base URL
    pub api_url: Option<String>,
    /// Access token
    pub access_token: Option<String>,
    /// Verify token for webhook
    pub verify_token: Option<String>,
    /// Phone number ID
    pub phone_number_id: Option<String>,
    /// Webhook listen port
    #[serde(default = "default_wa_port")]
    pub webhook_port: u16,
}

fn default_wa_port() -> u16 {
    8090
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    #[serde(default = "bool_true")]
    pub enabled: bool,
    /// Bot token (xoxb-...)
    pub bot_token: Option<String>,
    /// App-level token (xapp-...) for socket mode
    pub app_token: Option<String>,
    /// Signing secret for HTTP mode
    pub signing_secret: Option<String>,
    /// Socket mode (true) or HTTP events (false)
    #[serde(default = "bool_true")]
    pub socket_mode: bool,
}

fn bool_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronConfig {
    #[serde(default)]
    pub jobs: Vec<CronJob>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub id: String,
    pub schedule: String,
    pub task: String,
    /// Channel to deliver output to (optional)
    pub channel: Option<String>,
    /// Target chat/user ID for delivery
    pub target: Option<String>,
    #[serde(default = "bool_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeConfig {
    /// Node display name
    pub name: Option<String>,
    /// Beacon interval in seconds
    #[serde(default = "default_beacon_interval")]
    pub beacon_interval_secs: u64,
    /// State directory for presence files
    #[serde(default = "default_state_dir")]
    pub state_dir: PathBuf,
}

fn default_beacon_interval() -> u64 {
    30
}

/// Resolve a secret: if value starts with "env:", read from environment.
pub fn resolve_secret(value: &Option<String>, env_var: &str) -> Option<String> {
    if let Some(v) = value {
        if let Some(env_name) = v.strip_prefix("env:") {
            std::env::var(env_name).ok()
        } else {
            Some(v.clone())
        }
    } else {
        std::env::var(env_var).ok()
    }
}

/// Load config from file, falling back to defaults.
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
        })
    }
}
