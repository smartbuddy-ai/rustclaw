// Ported from zeroclaw src/providers/reliable.rs and src/providers/router.rs
use crate::auth::{self, ChatMessage, CompletionResponse};
use crate::config::{Config, ProviderRoute};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::Duration;

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn complete(
        &self,
        cfg: &Config,
        messages: &[ChatMessage],
        system: Option<&str>,
        model: &str,
    ) -> Result<CompletionResponse>;
}

pub struct AnthropicProvider;
pub struct OpenAiProvider;

#[async_trait]
impl CompletionProvider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }

    async fn complete(
        &self,
        _cfg: &Config,
        messages: &[ChatMessage],
        system: Option<&str>,
        model: &str,
    ) -> Result<CompletionResponse> {
        // BUG-06: Use resolve_api_key for env var -> credential store fallback
        let api_key = auth::resolve_api_key("ANTHROPIC_API_KEY", "anthropic_api_key")?;
        let auth = auth::LlmAuth {
            provider: "anthropic".into(),
            model: model.into(),
            api_key,
            base_url: "https://api.anthropic.com".into(),
        };
        auth::anthropic::complete(&auth, messages, system).await
    }
}

#[async_trait]
impl CompletionProvider for OpenAiProvider {
    fn name(&self) -> &str { "openai" }

    async fn complete(
        &self,
        _cfg: &Config,
        messages: &[ChatMessage],
        system: Option<&str>,
        model: &str,
    ) -> Result<CompletionResponse> {
        // BUG-06: Use resolve_api_key for env var -> credential store fallback
        let api_key = auth::resolve_api_key("OPENAI_API_KEY", "openai_api_key")?;
        let auth = auth::LlmAuth {
            provider: "openai".into(),
            model: model.into(),
            api_key,
            base_url: "https://api.openai.com".into(),
        };
        auth::openai::complete(&auth, messages, system).await
    }
}

#[derive(Debug, Clone)]
pub struct Router {
    routes: HashMap<String, ProviderRoute>,
    default_provider: String,
    default_model: String,
}

impl Router {
    pub fn from_config(cfg: &Config) -> Self {
        let mut routes = HashMap::new();
        for r in &cfg.auth.routes {
            routes.insert(r.hint.clone(), r.clone());
        }

        Self {
            routes,
            default_provider: cfg.auth.default_provider.clone(),
            default_model: cfg.auth.default_model.clone(),
        }
    }

    pub fn resolve(&self, model_or_hint: Option<&str>) -> (String, String) {
        match model_or_hint {
            Some(value) if value.starts_with("hint:") => {
                let hint = value.trim_start_matches("hint:");
                if let Some(route) = self.routes.get(hint) {
                    return (route.provider.clone(), route.model.clone());
                }
                (self.default_provider.clone(), self.default_model.clone())
            }
            Some(model) => (self.default_provider.clone(), model.to_string()),
            None => (self.default_provider.clone(), self.default_model.clone()),
        }
    }
}

pub struct ReliableRouter {
    providers: HashMap<String, Box<dyn CompletionProvider>>,
    router: Router,
    max_retries: u32,
    base_backoff_ms: u64,
    fallback_order: Vec<String>,
}

impl ReliableRouter {
    pub fn from_config(cfg: &Config) -> Self {
        let mut providers: HashMap<String, Box<dyn CompletionProvider>> = HashMap::new();
        providers.insert("anthropic".into(), Box::new(AnthropicProvider));
        providers.insert("openai".into(), Box::new(OpenAiProvider));

        Self {
            providers,
            router: Router::from_config(cfg),
            max_retries: cfg.auth.reliability.max_retries,
            base_backoff_ms: cfg.auth.reliability.base_backoff_ms,
            fallback_order: if cfg.auth.reliability.fallback_order.is_empty() {
                vec!["anthropic".into(), "openai".into()]
            } else {
                cfg.auth.reliability.fallback_order.clone()
            },
        }
    }

    pub async fn complete(
        &self,
        cfg: &Config,
        messages: &[ChatMessage],
        system: Option<&str>,
        model_or_hint: Option<&str>,
    ) -> Result<CompletionResponse> {
        let (primary_provider, model) = self.router.resolve(model_or_hint);
        let mut provider_order = vec![primary_provider.clone()];
        for provider in &self.fallback_order {
            if !provider_order.contains(provider) {
                provider_order.push(provider.clone());
            }
        }

        let mut failures = Vec::new();
        for provider_name in provider_order {
            let Some(provider) = self.providers.get(&provider_name) else {
                failures.push(format!("unknown provider: {provider_name}"));
                continue;
            };

            let mut backoff = self.base_backoff_ms.max(50);
            for attempt in 0..=self.max_retries {
                match provider.complete(cfg, messages, system, &model).await {
                    Ok(r) => return Ok(r),
                    Err(e) => {
                        let msg = e.to_string();
                        failures.push(format!(
                            "provider={provider_name} attempt={}/{} error={msg}",
                            attempt + 1,
                            self.max_retries + 1
                        ));

                        if attempt < self.max_retries && is_retryable(&msg) {
                            tokio::time::sleep(Duration::from_millis(backoff)).await;
                            backoff = (backoff * 2).min(10_000);
                            continue;
                        }
                        break;
                    }
                }
            }
        }

        anyhow::bail!("all providers failed:\n{}", failures.join("\n"))
    }
}

fn is_retryable(msg: &str) -> bool {
    !(msg.contains("401") || msg.contains("403") || msg.contains("404"))
}

// -- Feature 8: Model aliases --

/// Map of friendly aliases to canonical model IDs.
pub fn resolve_model_alias(model: &str) -> &str {
    match model {
        "claude-sonnet-4-6" | "claude-sonnet-4.6" => "claude-sonnet-4-20250514",
        "claude-opus-4-6" | "claude-opus-4.6" => "claude-opus-4-20250514",
        "claude-haiku-4-5" | "claude-haiku-4.5" => "claude-haiku-4-5-20251001",
        _ => model,
    }
}

/// Check if a model should use the 1M context window beta header.
pub fn needs_context_1m_header(model: &str) -> bool {
    // Models that support 1M context opt-in via beta header
    let canonical = resolve_model_alias(model);
    canonical.contains("claude-sonnet-4") || canonical.contains("claude-opus-4")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, ChannelsConfig, CronConfig, NodeConfig, ProviderReliabilityConfig};
    use std::path::PathBuf;

    fn test_cfg() -> Config {
        Config {
            workspace_dir: PathBuf::from("/tmp"),
            auth: AuthConfig {
                default_provider: "anthropic".into(),
                default_model: "claude-sonnet".into(),
                reliability: ProviderReliabilityConfig::default(),
                routes: vec![ProviderRoute {
                    hint: "fast".into(),
                    provider: "openai".into(),
                    model: "gpt-4o-mini".into(),
                }],
            },
            channels: ChannelsConfig::default(),
            cron: CronConfig::default(),
            node: NodeConfig::default(),
            memory: crate::config::MemoryConfig::default(),
            tunnel: crate::config::TunnelConfig::default(),
            gateway: crate::config::GatewayConfig::default(),
            tools: crate::config::ToolsConfig::default(),
        }
    }

    #[test]
    fn router_resolves_hint() {
        let router = Router::from_config(&test_cfg());
        let (provider, model) = router.resolve(Some("hint:fast"));
        assert_eq!(provider, "openai");
        assert_eq!(model, "gpt-4o-mini");
    }

    #[test]
    fn router_resolves_default() {
        let router = Router::from_config(&test_cfg());
        let (provider, model) = router.resolve(None);
        assert_eq!(provider, "anthropic");
        assert_eq!(model, "claude-sonnet");
    }

    // Feature 8: Model alias tests
    #[test]
    fn sonnet_46_alias_resolves() {
        assert_eq!(resolve_model_alias("claude-sonnet-4-6"), "claude-sonnet-4-20250514");
        assert_eq!(resolve_model_alias("claude-sonnet-4.6"), "claude-sonnet-4-20250514");
    }

    #[test]
    fn opus_46_alias_resolves() {
        assert_eq!(resolve_model_alias("claude-opus-4-6"), "claude-opus-4-20250514");
        assert_eq!(resolve_model_alias("claude-opus-4.6"), "claude-opus-4-20250514");
    }

    #[test]
    fn haiku_45_alias_resolves() {
        assert_eq!(resolve_model_alias("claude-haiku-4-5"), "claude-haiku-4-5-20251001");
    }

    #[test]
    fn unknown_model_passes_through() {
        assert_eq!(resolve_model_alias("gpt-4o"), "gpt-4o");
    }

    #[test]
    fn context_1m_header_needed_for_claude4() {
        assert!(needs_context_1m_header("claude-sonnet-4-6"));
        assert!(needs_context_1m_header("claude-opus-4-6"));
        assert!(!needs_context_1m_header("gpt-4o"));
    }
}
