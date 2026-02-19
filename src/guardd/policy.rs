use crate::guardd::{Action, FileAccessMode, Verdict};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Policy rule conditions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleConditions {
    /// Optional actor match.
    pub actor: Option<String>,
    /// Optional channel match for send actions.
    pub channel: Option<String>,
}

/// Policy rule definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Action pattern to match against the action label.
    pub action_pattern: String,
    /// Verdict to return on match.
    pub verdict: Verdict,
    /// Conditions that must be satisfied for a match.
    #[serde(default)]
    pub conditions: RuleConditions,
}

/// Rate limiting configuration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum actions per window.
    pub max_actions: usize,
    /// Window length in seconds.
    pub window_secs: u64,
}

/// Policy engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Optional policy rules.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// Workspace directory for implicit read allowlist.
    pub workspace_dir: PathBuf,
    /// Rate limit settings.
    pub rate_limit: Option<RateLimit>,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        let workspace_dir = directories::BaseDirs::new()
            .map(|d| d.home_dir().join(".rustclaw/workspace"))
            .unwrap_or_else(|| PathBuf::from(".rustclaw/workspace"));
        Self {
            rules: Vec::new(),
            workspace_dir,
            rate_limit: Some(RateLimit {
                max_actions: 120,
                window_secs: 60,
            }),
        }
    }
}

/// Policy engine evaluating actions against configured rules.
pub struct PolicyEngine {
    rules: Vec<Rule>,
    workspace_dir: PathBuf,
    rate_limit: Option<RateLimit>,
    recent: Mutex<VecDeque<Instant>>,
}

impl PolicyEngine {
    /// Build a policy engine from configuration.
    pub fn new(config: PolicyConfig) -> Self {
        Self {
            rules: config.rules,
            workspace_dir: config.workspace_dir,
            rate_limit: config.rate_limit,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    /// Evaluate an action and return a verdict.
    pub fn evaluate(&self, action: &Action) -> Verdict {
        if self.is_rate_limited() {
            return Verdict::AskHuman;
        }

        if self.is_dangerous_command(action) {
            return Verdict::Deny;
        }

        if self.is_workspace_read(action) {
            return Verdict::Allow;
        }

        for rule in &self.rules {
            if self.rule_matches(rule, action) {
                return rule.verdict;
            }
        }

        Verdict::Deny
    }

    fn is_rate_limited(&self) -> bool {
        let Some(limit) = self.rate_limit else {
            return false;
        };

        let mut recent = self.recent.lock().expect("rate limit lock");
        let now = Instant::now();
        let window = Duration::from_secs(limit.window_secs);
        while let Some(front) = recent.front() {
            if now.duration_since(*front) > window {
                recent.pop_front();
            } else {
                break;
            }
        }
        if recent.len() >= limit.max_actions {
            return true;
        }
        recent.push_back(now);
        false
    }

    fn is_dangerous_command(&self, action: &Action) -> bool {
        let Action::RunCommand { command } = action else {
            return false;
        };
        let lowered = command.to_lowercase();
        lowered.contains("rm ") || lowered.contains("sudo")
    }

    fn is_workspace_read(&self, action: &Action) -> bool {
        let Action::AccessFile { path, mode } = action else {
            return false;
        };
        if *mode != FileAccessMode::Read {
            return false;
        }
        if path.is_absolute() {
            path.starts_with(&self.workspace_dir)
        } else {
            true
        }
    }

    fn rule_matches(&self, rule: &Rule, action: &Action) -> bool {
        let label = action.label();
        let Ok(pattern) = Pattern::new(&rule.action_pattern) else {
            return false;
        };
        if !pattern.matches(&label) {
            return false;
        }

        if let Some(actor) = &rule.conditions.actor {
            if actor != "system" {
                return false;
            }
        }

        if let Some(channel) = &rule.conditions.channel {
            if let Action::SendMessage { channel: action_channel, .. } = action {
                if action_channel != channel {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guardd::{Action, FileAccessMode, Verdict};
    use tempfile::TempDir;

    #[test]
    fn default_denies_unknown() {
        let config = PolicyConfig::default();
        let engine = PolicyEngine::new(config);
        let verdict = engine.evaluate(&Action::ApiCall {
            method: "GET".into(),
            endpoint: "https://example.com".into(),
        });
        assert_eq!(verdict, Verdict::Deny);
    }

    #[test]
    fn allows_workspace_read() {
        let temp = TempDir::new().expect("tempdir");
        let config = PolicyConfig {
            rules: Vec::new(),
            workspace_dir: temp.path().to_path_buf(),
            rate_limit: None,
        };
        let engine = PolicyEngine::new(config);
        let verdict = engine.evaluate(&Action::AccessFile {
            path: temp.path().join("doc.md"),
            mode: FileAccessMode::Read,
        });
        assert_eq!(verdict, Verdict::Allow);
    }

    #[test]
    fn denies_rm_or_sudo() {
        let config = PolicyConfig {
            rules: Vec::new(),
            workspace_dir: PathBuf::from("/tmp"),
            rate_limit: None,
        };
        let engine = PolicyEngine::new(config);
        let verdict = engine.evaluate(&Action::RunCommand {
            command: "sudo rm -rf /".into(),
        });
        assert_eq!(verdict, Verdict::Deny);
    }

    #[test]
    fn rate_limit_asks_human() {
        let config = PolicyConfig {
            rules: Vec::new(),
            workspace_dir: PathBuf::from("/tmp"),
            rate_limit: Some(RateLimit {
                max_actions: 1,
                window_secs: 60,
            }),
        };
        let engine = PolicyEngine::new(config);
        let _ = engine.evaluate(&Action::WebhookInbound {
            source: "slack".into(),
        });
        let verdict = engine.evaluate(&Action::WebhookInbound {
            source: "slack".into(),
        });
        assert_eq!(verdict, Verdict::AskHuman);
    }
}
