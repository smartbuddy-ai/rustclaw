pub mod anthropic;
pub mod openai;
pub mod retry;

use crate::config::Config;
// retry kept in module for legacy compatibility
use serde::{Deserialize, Serialize};

/// Resolved API credentials.
#[derive(Debug, Clone)]
pub struct LlmAuth {
    pub provider: String,
    pub model: String,
    pub api_key: String,
    pub base_url: String,
}

/// Chat message for LLM APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// LLM completion response.
#[derive(Debug, Clone)]
pub struct CompletionResponse {
    pub content: String,
    pub model: String,
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

/// Resolve LLM auth from config + environment.
/// API keys are loaded from env vars (populated by dotenvy from ~/.rustclaw/.env).
pub fn resolve_auth(cfg: &Config) -> anyhow::Result<LlmAuth> {
    let provider = &cfg.auth.default_provider;

    match provider.as_str() {
        "anthropic" => {
            let api_key = resolve_api_key("ANTHROPIC_API_KEY", "anthropic_api_key")?;
            Ok(LlmAuth {
                provider: "anthropic".into(),
                model: cfg.auth.default_model.clone(),
                api_key,
                base_url: "https://api.anthropic.com".into(),
            })
        }
        "openai" => {
            let api_key = resolve_api_key("OPENAI_API_KEY", "openai_api_key")?;
            Ok(LlmAuth {
                provider: "openai".into(),
                model: cfg.auth.default_model.clone(),
                api_key,
                base_url: "https://api.openai.com".into(),
            })
        }
        _ => anyhow::bail!("Unknown provider: {provider}. Use 'anthropic' or 'openai'."),
    }
}

/// BUG-06: Resolve an API key by trying env var first, then credential store as fallback.
/// Order: 1) env var, 2) credential store (guardd AES-GCM encrypted).
pub fn resolve_api_key(env_var: &str, cred_name: &str) -> anyhow::Result<String> {
    // Try environment variable first
    if let Ok(val) = std::env::var(env_var) {
        if !val.is_empty() {
            return Ok(val);
        }
    }

    // Fallback to credential store
    let cred_path = directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/credentials.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".rustclaw/credentials.json"));

    let key_path = directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/.cred_key"))
        .unwrap_or_else(|| std::path::PathBuf::from(".rustclaw/.cred_key"));

    if cred_path.exists() && key_path.exists() {
        if let Ok(key_bytes) = std::fs::read(&key_path) {
            if key_bytes.len() == 32 {
                let key = crate::guardd::credentials::Secret::new(key_bytes);
                if let Ok(store) = crate::guardd::credentials::CredentialStore::new(cred_path, key) {
                    if let Ok(Some(secret)) = store.retrieve(cred_name) {
                        if let Ok(val) = String::from_utf8(secret.as_bytes().to_vec()) {
                            if !val.is_empty() {
                                return Ok(val);
                            }
                        }
                    }
                }
            }
        }
    }

    anyhow::bail!("{env_var} not set and not found in credential store. Run `rustclaw init` or set the env var.")
}

/// Send a chat completion request to the configured LLM with retry logic.
pub async fn complete(
    cfg: &Config,
    messages: &[ChatMessage],
    system: Option<&str>,
) -> anyhow::Result<CompletionResponse> {
    // Ported architecture from zeroclaw src/providers/reliable.rs + router.rs
    let router = crate::providers::ReliableRouter::from_config(cfg);
    router
        .complete(cfg, messages, system, Some(&cfg.auth.default_model))
        .await
}
