use crate::guardd::{Action, Verdict};
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

/// Audit log entry written to JSONL.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// RFC3339 timestamp.
    pub timestamp: String,
    /// Action label.
    pub action: String,
    /// Verdict returned.
    pub verdict: Verdict,
    /// Actor identifier.
    pub actor: String,
    /// Additional metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Append-only audit logger for guardd.
#[derive(Debug, Clone)]
pub struct AuditLogger {
    path: PathBuf,
}

impl AuditLogger {
    /// Create a new audit logger.
    pub fn new(path: Option<PathBuf>) -> Result<Self> {
        let path = path.unwrap_or_else(default_audit_path);
        Ok(Self { path })
    }

    /// Log an action and verdict.
    pub fn log_action(
        &self,
        action: &Action,
        verdict: Verdict,
        actor: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let entry = AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            action: action.label(),
            verdict,
            actor: actor.to_string(),
            metadata,
        };
        self.write_entry(&entry)
    }

    /// Write a raw audit entry.
    pub fn write_entry(&self, entry: &AuditEntry) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating audit dir {}", parent.display()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening audit log {}", self.path.display()))?;
        let line = serde_json::to_string(entry).context("encoding audit entry")?;
        writeln!(file, "{line}").context("writing audit entry")
    }
}

fn default_audit_path() -> PathBuf {
    directories::BaseDirs::new()
        .map(|d| d.home_dir().join(".rustclaw/audit.jsonl"))
        .unwrap_or_else(|| PathBuf::from(".rustclaw/audit.jsonl"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guardd::{Action, Verdict};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn writes_jsonl_entry() {
        let temp = TempDir::new().expect("tempdir");
        let logger = AuditLogger::new(Some(temp.path().join("audit.jsonl"))).expect("logger");
        logger
            .log_action(
                &Action::WebhookInbound {
                    source: "slack".into(),
                },
                Verdict::Allow,
                "tester",
                None,
            )
            .expect("log");
        let content = fs::read_to_string(temp.path().join("audit.jsonl")).expect("read");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1);
        let entry: AuditEntry = serde_json::from_str(lines[0]).expect("parse");
        assert_eq!(entry.actor, "tester");
    }

    #[test]
    fn appends_entries() {
        let temp = TempDir::new().expect("tempdir");
        let logger = AuditLogger::new(Some(temp.path().join("audit.jsonl"))).expect("logger");
        logger
            .log_action(
                &Action::RunCommand {
                    command: "ls".into(),
                },
                Verdict::Allow,
                "tester",
                None,
            )
            .expect("log");
        logger
            .log_action(
                &Action::RunCommand {
                    command: "git status".into(),
                },
                Verdict::AskHuman,
                "tester",
                None,
            )
            .expect("log");
        let content = fs::read_to_string(temp.path().join("audit.jsonl")).expect("read");
        assert_eq!(content.lines().count(), 2);
    }

    #[test]
    fn preserves_metadata() {
        let temp = TempDir::new().expect("tempdir");
        let logger = AuditLogger::new(Some(temp.path().join("audit.jsonl"))).expect("logger");
        let meta = serde_json::json!({"ip": "127.0.0.1"});
        logger
            .log_action(
                &Action::ApiCall {
                    method: "GET".into(),
                    endpoint: "https://example.com".into(),
                },
                Verdict::Allow,
                "tester",
                Some(meta),
            )
            .expect("log");
        let content = fs::read_to_string(temp.path().join("audit.jsonl")).expect("read");
        let entry: AuditEntry = serde_json::from_str(content.lines().next().expect("line"))
            .expect("parse");
        assert!(entry.metadata.is_some());
    }
}
