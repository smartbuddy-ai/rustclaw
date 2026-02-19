use crate::guardd::{Action, FileAccessMode, GuardConfig, GuardDaemon, Verdict};
use crate::guardd::policy::{Rule, RuleConditions};
use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub struct FileTool {
    workspace: PathBuf,
}

impl FileTool {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }

    fn checked_path(&self, path: &str, write: bool) -> Result<PathBuf> {
        let p = if Path::new(path).is_absolute() { PathBuf::from(path) } else { self.workspace.join(path) };
        if !p.starts_with(&self.workspace) {
            return Err(anyhow!("path escapes workspace"));
        }
        let mut cfg = GuardConfig::default();
        cfg.policy.workspace_dir = self.workspace.clone();
        cfg.policy.rules.push(Rule { action_pattern: "file:write:*".into(), verdict: Verdict::Allow, conditions: RuleConditions::default() });
        let guard = GuardDaemon::new(cfg)?;
        let verdict = guard.authorize(&Action::AccessFile { path: p.clone(), mode: if write { FileAccessMode::Write } else { FileAccessMode::Read } });
        match verdict {
            Verdict::Allow => Ok(p),
            Verdict::Deny => Err(anyhow!("blocked by guardd")),
            Verdict::AskHuman => Err(anyhow!("needs human approval")),
        }
    }
}

#[derive(Deserialize)]
struct FileReq {
    action: String,
    path: Option<String>,
    content: Option<String>,
}

#[async_trait]
impl Tool for FileTool {
    fn name(&self) -> &'static str { "file" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: FileReq = serde_json::from_value(input)?;
        match req.action.as_str() {
            "read" => {
                let path = self.checked_path(req.path.as_deref().ok_or_else(|| anyhow!("path required"))?, false)?;
                let content = std::fs::read_to_string(path)?;
                Ok(json!({"content": content}))
            }
            "write" => {
                let path = self.checked_path(req.path.as_deref().ok_or_else(|| anyhow!("path required"))?, true)?;
                if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
                std::fs::write(path, req.content.unwrap_or_default())?;
                Ok(json!({"ok": true}))
            }
            "list" => {
                let path = self.checked_path(req.path.as_deref().unwrap_or("."), false)?;
                let mut out = vec![];
                for entry in std::fs::read_dir(path)? {
                    let e = entry?;
                    out.push(json!({
                        "name": e.file_name().to_string_lossy(),
                        "is_dir": e.path().is_dir()
                    }));
                }
                Ok(json!({"entries": out}))
            }
            _ => Err(anyhow!("unsupported action")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn file_write_read_list() {
        let tmp = TempDir::new().unwrap();
        let tool = FileTool::new(tmp.path().to_path_buf());
        tool.run(json!({"action":"write","path":"a.txt","content":"hi"})).await.unwrap();
        let out = tool.run(json!({"action":"read","path":"a.txt"})).await.unwrap();
        assert_eq!(out["content"], "hi");
        let list = tool.run(json!({"action":"list","path":"."})).await.unwrap();
        assert!(list["entries"].as_array().unwrap().iter().any(|e| e["name"] == "a.txt"));
    }
}
