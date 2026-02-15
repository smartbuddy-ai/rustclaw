pub mod anthropic;
pub mod openai;

use crate::config::Config;
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
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!(
                    "ANTHROPIC_API_KEY not set. Run `rustclaw init` or set the env var."
                ))?;
            Ok(LlmAuth {
                provider: "anthropic".into(),
                model: cfg.auth.default_model.clone(),
                api_key,
                base_url: "https://api.anthropic.com".into(),
            })
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .ok()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow::anyhow!(
                    "OPENAI_API_KEY not set. Run `rustclaw init` or set the env var."
                ))?;
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

/// Send a chat completion request to the configured LLM.
pub async fn complete(
    cfg: &Config,
    messages: &[ChatMessage],
    system: Option<&str>,
) -> anyhow::Result<CompletionResponse> {
    let auth = resolve_auth(cfg)?;
    match auth.provider.as_str() {
        "anthropic" => anthropic::complete(&auth, messages, system).await,
        "openai" => openai::complete(&auth, messages, system).await,
        _ => unreachable!(),
    }
}
