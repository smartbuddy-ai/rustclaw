use crate::guardd::Verdict;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Sandbox configuration for command and path filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Allowed path patterns.
    #[serde(default)]
    pub allowed_paths: Vec<String>,
    /// Allowed command prefixes.
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    /// Denied patterns for commands or paths.
    #[serde(default)]
    pub denied_patterns: Vec<String>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            allowed_paths: Vec::new(),
            allowed_commands: vec![
                "cargo".to_string(),
                "git".to_string(),
                "cat".to_string(),
                "ls".to_string(),
                "echo".to_string(),
                "python".to_string(),
                "python3".to_string(),
            ],
            denied_patterns: vec![
                "rm -rf".to_string(),
                "sudo".to_string(),
                "chmod 777".to_string(),
                "curl|sh".to_string(),
                "curl | sh".to_string(),
            ],
        }
    }
}

/// Sandbox enforcement for commands and file paths.
#[derive(Debug, Clone)]
pub struct Sandbox {
    allowed_paths: Vec<String>,
    allowed_commands: Vec<String>,
    denied_patterns: Vec<String>,
}

impl Sandbox {
    /// Build a sandbox from configuration.
    pub fn new(config: SandboxConfig) -> Self {
        Self {
            allowed_paths: config.allowed_paths,
            allowed_commands: config.allowed_commands,
            denied_patterns: config.denied_patterns,
        }
    }

    /// Check whether a command is allowed in the sandbox.
    pub fn check_command(&self, command: &str) -> Verdict {
        let lowered = command.to_lowercase();
        if self
            .denied_patterns
            .iter()
            .any(|pattern| lowered.contains(pattern))
        {
            return Verdict::Deny;
        }

        let command_name = command
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_lowercase();
        if self
            .allowed_commands
            .iter()
            .any(|allowed| allowed == &command_name)
        {
            Verdict::Allow
        } else {
            Verdict::AskHuman
        }
    }

    /// Check whether a path is allowed in the sandbox.
    pub fn check_path(&self, path: &Path) -> Verdict {
        let path_str = path.to_string_lossy();
        if self
            .denied_patterns
            .iter()
            .any(|pattern| path_str.contains(pattern))
        {
            return Verdict::Deny;
        }

        if self.allowed_paths.is_empty() {
            return Verdict::AskHuman;
        }

        for allowed in &self.allowed_paths {
            if let Ok(pattern) = Pattern::new(allowed) {
                if pattern.matches(&path_str) {
                    return Verdict::Allow;
                }
            }
        }

        Verdict::AskHuman
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guardd::Verdict;

    #[test]
    fn denies_rm_rf() {
        let sandbox = Sandbox::new(SandboxConfig::default());
        let verdict = sandbox.check_command("rm -rf /");
        assert_eq!(verdict, Verdict::Deny);
    }

    #[test]
    fn allows_known_command() {
        let sandbox = Sandbox::new(SandboxConfig::default());
        let verdict = sandbox.check_command("git status");
        assert_eq!(verdict, Verdict::Allow);
    }

    #[test]
    fn path_allowlist_matches() {
        let mut config = SandboxConfig::default();
        config.allowed_paths = vec!["/tmp/**".to_string()];
        let sandbox = Sandbox::new(config);
        let verdict = sandbox.check_path(Path::new("/tmp/data.txt"));
        assert_eq!(verdict, Verdict::Allow);
    }

    #[test]
    fn unknown_command_prompts_human() {
        let sandbox = Sandbox::new(SandboxConfig::default());
        let verdict = sandbox.check_command("terraform apply");
        assert_eq!(verdict, Verdict::AskHuman);
    }
}
