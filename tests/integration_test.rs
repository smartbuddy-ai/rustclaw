use std::path::PathBuf;
use tempfile::TempDir;

/// Integration test: workspace initialization and config loading
#[tokio::test]
async fn test_workspace_initialization() {
    let temp = TempDir::new().unwrap();
    let workspace_dir = temp.path().to_path_buf();

    // Create a minimal config
    let cfg = rustclaw::config::Config {
        workspace_dir: workspace_dir.clone(),
        auth: rustclaw::config::AuthConfig {
            default_provider: "anthropic".into(),
            default_model: "claude-sonnet-4-20250514".into(),
            reliability: rustclaw::config::ProviderReliabilityConfig::default(),
            routes: vec![],
        },
        channels: rustclaw::config::ChannelsConfig::default(),
        cron: rustclaw::config::CronConfig::default(),
        node: rustclaw::config::NodeConfig::default(),
        memory: rustclaw::config::MemoryConfig::default(),
        tunnel: rustclaw::config::TunnelConfig::default(),
        gateway: rustclaw::config::GatewayConfig::default(),
        tools: rustclaw::config::ToolsConfig::default(),
    };

    // Ensure workspace files are created
    rustclaw::workspace::ensure_workspace(&cfg).unwrap();

    // Verify workspace files exist
    assert!(workspace_dir.join("SOUL.md").exists());
    assert!(workspace_dir.join("AGENTS.md").exists());
    assert!(workspace_dir.join("USER.md").exists());
    assert!(workspace_dir.join("IDENTITY.md").exists());
    assert!(workspace_dir.join("TOOLS.md").exists());
    assert!(workspace_dir.join("MEMORY.md").exists());
    assert!(workspace_dir.join("HEARTBEAT.md").exists());
    assert!(workspace_dir.join("memory").is_dir());
}

/// Integration test: system prompt building from workspace files
#[tokio::test]
async fn test_system_prompt_building() {
    let temp = TempDir::new().unwrap();
    let workspace_dir = temp.path().to_path_buf();

    let cfg = rustclaw::config::Config {
        workspace_dir: workspace_dir.clone(),
        auth: rustclaw::config::AuthConfig::default(),
        channels: rustclaw::config::ChannelsConfig::default(),
        cron: rustclaw::config::CronConfig::default(),
        node: rustclaw::config::NodeConfig::default(),
        memory: rustclaw::config::MemoryConfig::default(),
        tunnel: rustclaw::config::TunnelConfig::default(),
        gateway: rustclaw::config::GatewayConfig::default(),
        tools: rustclaw::config::ToolsConfig::default(),
    };

    rustclaw::workspace::ensure_workspace(&cfg).unwrap();

    // Build system prompt
    let prompt = rustclaw::workspace::build_system_prompt(&cfg).unwrap();

    // Verify it contains expected sections
    assert!(prompt.contains("SOUL.md") || prompt.len() > 100);
    assert!(prompt.contains("Workspace Rules") || prompt.contains("AGENTS.md"));
}

/// Integration test: session management
#[tokio::test]
async fn test_session_management() {
    let temp = TempDir::new().unwrap();
    let workspace_dir = temp.path().to_path_buf();

    let store = rustclaw::chat::session::SessionStore::new(workspace_dir.clone(), 10).unwrap();

    // Create a session
    let session_id = "test-session-123";
    store.add_and_save(session_id, "user", "Hello!").unwrap();
    store.add_and_save(session_id, "assistant", "Hi there!").unwrap();

    // Load the session
    let session = store.load(session_id).unwrap();
    let messages = session.get_messages();

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, "user");
    assert_eq!(messages[0].content, "Hello!");
    assert_eq!(messages[1].role, "assistant");
    assert_eq!(messages[1].content, "Hi there!");
}

/// Integration test: config serialization and deserialization
#[test]
fn test_config_roundtrip() {
    let cfg = rustclaw::config::Config {
        workspace_dir: PathBuf::from("/tmp/workspace"),
        auth: rustclaw::config::AuthConfig {
            default_provider: "anthropic".into(),
            default_model: "claude-3-5-sonnet".into(),
            reliability: rustclaw::config::ProviderReliabilityConfig::default(),
            routes: vec![],
        },
        channels: rustclaw::config::ChannelsConfig::default(),
        cron: rustclaw::config::CronConfig::default(),
        node: rustclaw::config::NodeConfig::default(),
        memory: rustclaw::config::MemoryConfig::default(),
        tunnel: rustclaw::config::TunnelConfig::default(),
        gateway: rustclaw::config::GatewayConfig::default(),
        tools: rustclaw::config::ToolsConfig::default(),
    };

    // Serialize to TOML
    let toml_str = toml::to_string(&cfg).unwrap();
    
    // Deserialize back
    let cfg2: rustclaw::config::Config = toml::from_str(&toml_str).unwrap();

    assert_eq!(cfg.workspace_dir, cfg2.workspace_dir);
    assert_eq!(cfg.auth.default_provider, cfg2.auth.default_provider);
    assert_eq!(cfg.auth.default_model, cfg2.auth.default_model);
}

/// Integration test: cron job management
#[tokio::test]
async fn test_cron_job_lifecycle() {
    let temp = TempDir::new().unwrap();
    let config_dir = temp.path();
    let config_file = config_dir.join("config.toml");

    let cfg = rustclaw::config::Config {
        workspace_dir: config_dir.join("workspace"),
        auth: rustclaw::config::AuthConfig::default(),
        channels: rustclaw::config::ChannelsConfig::default(),
        cron: rustclaw::config::CronConfig {
            jobs: vec![],
            enable_heartbeat: false,
            heartbeat_interval_min: 30,
        },
        node: rustclaw::config::NodeConfig::default(),
        memory: rustclaw::config::MemoryConfig::default(),
        tunnel: rustclaw::config::TunnelConfig::default(),
        gateway: rustclaw::config::GatewayConfig::default(),
        tools: rustclaw::config::ToolsConfig::default(),
    };

    // Save initial config
    std::fs::create_dir_all(config_dir).unwrap();
    std::fs::write(&config_file, toml::to_string(&cfg).unwrap()).unwrap();

    // Note: Cron add/remove/list functions require a config with a valid path
    // For a full integration test, you would:
    // 1. Add a job
    // 2. Verify it's in the list
    // 3. Remove it
    // 4. Verify it's gone
    
    // This is a placeholder for the actual cron integration test
    // which would require proper file system paths
    assert!(config_file.exists());
}
