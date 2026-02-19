#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_config_nonexistent() {
        let result = load_config(Some(&PathBuf::from("/nonexistent/config.toml")));
        assert!(result.is_ok());
        let cfg = result.unwrap();
        assert!(cfg.workspace_dir.to_string_lossy().contains(".rustclaw"));
        assert_eq!(cfg.auth.reliability.max_retries, 2);
        assert_eq!(cfg.memory.backend, "sqlite");
        assert_eq!(cfg.tunnel.provider, "none");
    }

    #[test]
    fn test_config_serialization() {
        let cfg = Config {
            workspace_dir: PathBuf::from("/test/workspace"),
            auth: AuthConfig {
                default_provider: "openai".into(),
                default_model: "gpt-4o".into(),
                reliability: ProviderReliabilityConfig::default(),
                routes: vec![],
            },
            channels: ChannelsConfig::default(),
            cron: CronConfig::default(),
            node: NodeConfig::default(),
            memory: MemoryConfig::default(),
            tunnel: TunnelConfig::default(),
            gateway: GatewayConfig::default(),
            tools: ToolsConfig::default(),
        };

        let toml_str = toml::to_string(&cfg).unwrap();
        assert!(toml_str.contains("openai"));
        assert!(toml_str.contains("gpt-4o"));

        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.auth.default_provider, "openai");
    }

    #[test]
    fn test_provider_route() {
        let route = ProviderRoute {
            hint: "fast".into(),
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
        };
        assert_eq!(route.hint, "fast");
    }
}
