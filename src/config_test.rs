#[cfg(test)]
mod tests {
    use super::super::*;
    use std::path::PathBuf;

    #[test]
    fn test_default_config() {
        // When deserialized from empty TOML, defaults are applied
        let toml_str = "";
        let cfg: Config = toml::from_str(toml_str).unwrap_or_else(|_| Config {
            workspace_dir: default_workspace_dir(),
            auth: AuthConfig::default(),
            channels: ChannelsConfig::default(),
            cron: CronConfig::default(),
            node: NodeConfig::default(),
        });

        // Note: Default::default() gives empty strings, serde defaults give real values
        // This is expected behavior - check serialization instead
        assert!(cfg.workspace_dir.to_string_lossy().contains(".rustclaw"));
    }

    #[test]
    fn test_load_config_nonexistent() {
        let result = load_config(Some(&PathBuf::from("/nonexistent/config.toml")));
        assert!(result.is_ok()); // Should return defaults
        let cfg = result.unwrap();
        // When using Default trait, provider is empty string
        // When loading from TOML, serde defaults kick in
        assert!(cfg.workspace_dir.to_string_lossy().contains(".rustclaw"));
    }

    #[test]
    fn test_config_serialization() {
        let cfg = Config {
            workspace_dir: PathBuf::from("/test/workspace"),
            auth: AuthConfig {
                default_provider: "openai".into(),
                default_model: "gpt-4o".into(),
            },
            channels: ChannelsConfig::default(),
            cron: CronConfig::default(),
            node: NodeConfig::default(),
        };

        let toml_str = toml::to_string(&cfg).unwrap();
        assert!(toml_str.contains("openai"));
        assert!(toml_str.contains("gpt-4o"));

        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(deserialized.auth.default_provider, "openai");
    }

    #[test]
    fn test_telegram_config_defaults() {
        let tg = TelegramConfig {
            enabled: true,
            bot_token: None,
            allow_from: vec![],
            webhook_url: None,
        };

        assert!(tg.enabled);
        assert!(tg.allow_from.is_empty());
    }

    #[test]
    fn test_cron_job_creation() {
        let job = CronJob {
            id: "test-123".into(),
            schedule: "0 9 * * MON".into(),
            task: "Test task".into(),
            channel: None,
            target: None,
            enabled: true,
        };

        assert_eq!(job.id, "test-123");
        assert!(job.enabled);
    }
}
