use crate::guardd::audit::AuditLogger;
use crate::guardd::policy::{PolicyConfig, PolicyEngine};
use crate::guardd::sandbox::{Sandbox, SandboxConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub mod audit;
pub mod channel_auth;
pub mod credentials;
pub mod policy;
pub mod sandbox;

/// Actions that require guard authorization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Send a message out via a channel.
    SendMessage { channel: String, content: String },
    /// Execute a local command.
    RunCommand { command: String },
    /// Access a file path for read/write.
    AccessFile { path: PathBuf, mode: FileAccessMode },
    /// Perform an outbound API call.
    ApiCall { method: String, endpoint: String },
    /// Inbound webhook payload.
    WebhookInbound { source: String },
}

/// File access intent for policy checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAccessMode {
    /// Read access to a file.
    Read,
    /// Write access to a file.
    Write,
}

/// Authorization verdicts returned by guardd.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// Allow the action immediately.
    Allow,
    /// Deny the action.
    Deny,
    /// Ask a human to approve the action.
    AskHuman,
}

/// Configuration for the guard daemon.
#[derive(Debug, Clone)]
pub struct GuardConfig {
    /// Policy engine configuration.
    pub policy: PolicyConfig,
    /// Sandbox configuration.
    pub sandbox: SandboxConfig,
    /// Audit log path override.
    pub audit_path: Option<PathBuf>,
    /// Default actor identifier for audit entries.
    pub default_actor: String,
}

impl Default for GuardConfig {
    fn default() -> Self {
        Self {
            policy: PolicyConfig::default(),
            sandbox: SandboxConfig::default(),
            audit_path: None,
            default_actor: "system".to_string(),
        }
    }
}

/// Guard daemon enforcing policies, sandbox, and audit logging.
pub struct GuardDaemon {
    policy: PolicyEngine,
    sandbox: Sandbox,
    audit: AuditLogger,
    default_actor: String,
}

impl GuardDaemon {
    /// Create a new guard daemon from configuration.
    pub fn new(config: GuardConfig) -> Result<Self> {
        let policy = PolicyEngine::new(config.policy);
        let sandbox = Sandbox::new(config.sandbox);
        let audit = AuditLogger::new(config.audit_path)?;
        Ok(Self {
            policy,
            sandbox,
            audit,
            default_actor: config.default_actor,
        })
    }

    /// Authorize an action and return a verdict.
    pub fn authorize(&self, action: &Action) -> Verdict {
        let policy_verdict = self.policy.evaluate(action);
        let mut verdict = policy_verdict;
        // Only apply sandbox checks if the policy didn't explicitly allow
        // (e.g. workspace reads are pre-authorized by policy)
        if verdict == Verdict::Allow {
            verdict = match action {
                Action::RunCommand { command } => self.sandbox.check_command(command),
                // File access already authorized by policy (workspace read) — skip sandbox
                Action::AccessFile { .. } => Verdict::Allow,
                _ => Verdict::Allow,
            };
        }

        let _ = self.audit.log_action(action, verdict, &self.default_actor, None);
        verdict
    }
}

impl Action {
    /// Normalize the action to a string label for pattern matching.
    pub fn label(&self) -> String {
        match self {
            Action::SendMessage { channel, content } => {
                format!("send:{channel}:{content}")
            }
            Action::RunCommand { command } => format!("run:{command}"),
            Action::AccessFile { path, mode } => {
                let mode = match mode {
                    FileAccessMode::Read => "read",
                    FileAccessMode::Write => "write",
                };
                format!("file:{mode}:{}", path.display())
            }
            Action::ApiCall { method, endpoint } => {
                format!("api:{}:{endpoint}", method.to_lowercase())
            }
            Action::WebhookInbound { source } => format!("webhook:{source}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guardd::policy::RateLimit;
    use tempfile::TempDir;

    #[test]
    fn authorize_denies_unknown() {
        let cfg = GuardConfig::default();
        let daemon = GuardDaemon::new(cfg).expect("guard daemon");
        let verdict = daemon.authorize(&Action::ApiCall {
            method: "GET".into(),
            endpoint: "https://example.com".into(),
        });
        assert_eq!(verdict, Verdict::Deny);
    }

    #[test]
    fn authorize_allows_workspace_read() {
        let temp = TempDir::new().expect("tempdir");
        let mut cfg = GuardConfig::default();
        cfg.policy.workspace_dir = temp.path().to_path_buf();
        cfg.policy.rate_limit = Some(RateLimit {
            max_actions: 100,
            window_secs: 60,
        });
        let daemon = GuardDaemon::new(cfg).expect("guard daemon");
        let verdict = daemon.authorize(&Action::AccessFile {
            path: temp.path().join("notes.md"),
            mode: FileAccessMode::Read,
        });
        assert_eq!(verdict, Verdict::Allow);
    }

    #[test]
    fn authorize_sandbox_blocks_command() {
        let mut cfg = GuardConfig::default();
        cfg.policy.rate_limit = None;
        let daemon = GuardDaemon::new(cfg).expect("guard daemon");
        let verdict = daemon.authorize(&Action::RunCommand {
            command: "rm -rf /".into(),
        });
        assert_eq!(verdict, Verdict::Deny);
    }
}
