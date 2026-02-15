#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::config::Config;
    use std::fs;
    use tempfile::TempDir;

    fn test_config(workspace_dir: std::path::PathBuf) -> Config {
        Config {
            workspace_dir,
            auth: crate::config::AuthConfig::default(),
            channels: crate::config::ChannelsConfig::default(),
            cron: crate::config::CronConfig::default(),
            node: crate::config::NodeConfig::default(),
        }
    }

    #[test]
    fn test_ensure_workspace() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());

        let result = ensure_workspace(&cfg);
        assert!(result.is_ok());

        // Check that workspace files exist
        assert!(temp.path().join("SOUL.md").exists());
        assert!(temp.path().join("AGENTS.md").exists());
        assert!(temp.path().join("USER.md").exists());
        assert!(temp.path().join("IDENTITY.md").exists());
        assert!(temp.path().join("TOOLS.md").exists());
        assert!(temp.path().join("MEMORY.md").exists());
        assert!(temp.path().join("HEARTBEAT.md").exists());

        // Check that memory directory exists
        assert!(temp.path().join("memory").exists());
    }

    #[test]
    fn test_read_nonexistent_file() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());

        let result = read_file(&cfg, "nonexistent.md").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_read_existing_file() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());

        ensure_workspace(&cfg).unwrap();

        let soul = read_file(&cfg, "SOUL.md").unwrap();
        assert!(soul.is_some());
        assert!(soul.unwrap().contains("You are"));
    }

    #[test]
    fn test_write_and_read_file() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());

        let content = "Test content for memory";
        write_file(&cfg, "test_memory.md", content).unwrap();

        let read_back = read_file(&cfg, "test_memory.md").unwrap();
        assert_eq!(read_back, Some(content.to_string()));
    }

    #[test]
    fn test_build_system_prompt() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());

        ensure_workspace(&cfg).unwrap();

        let prompt = build_system_prompt(&cfg).unwrap();
        assert!(prompt.contains("You are")); // From SOUL.md
        assert!(prompt.contains("Workspace Rules")); // From AGENTS.md
    }

    #[test]
    fn test_workspace_files_not_overwritten() {
        let temp = TempDir::new().unwrap();
        let cfg = test_config(temp.path().to_path_buf());

        ensure_workspace(&cfg).unwrap();

        // Modify a file
        let custom_content = "Custom SOUL content";
        fs::write(temp.path().join("SOUL.md"), custom_content).unwrap();

        // Ensure workspace again - should not overwrite
        ensure_workspace(&cfg).unwrap();

        let soul = read_file(&cfg, "SOUL.md").unwrap().unwrap();
        assert_eq!(soul, custom_content);
    }
}
