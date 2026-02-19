use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Background process manager — tracks running exec sessions.
pub struct ProcessManager {
    sessions: Arc<RwLock<HashMap<String, ProcessSession>>>,
}

struct ProcessSession {
    id: String,
    command: String,
    started_at: chrono::DateTime<chrono::Utc>,
    status: ProcessStatus,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

#[derive(Clone, Debug, PartialEq)]
enum ProcessStatus {
    Running,
    Completed,
    Failed,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

pub struct ProcessTool {
    manager: Arc<ProcessManager>,
}

impl ProcessTool {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(ProcessManager::new()),
        }
    }
}

#[derive(Deserialize)]
struct ProcessReq {
    action: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ProcessTool {
    fn name(&self) -> &'static str { "process" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: ProcessReq = serde_json::from_value(input)?;
        match req.action.as_str() {
            "list" => {
                let sessions = self.manager.sessions.read().await;
                let list: Vec<Value> = sessions.values().map(|s| {
                    json!({
                        "id": s.id,
                        "command": s.command,
                        "started_at": s.started_at.to_rfc3339(),
                        "status": match s.status {
                            ProcessStatus::Running => "running",
                            ProcessStatus::Completed => "completed",
                            ProcessStatus::Failed => "failed",
                        },
                        "exit_code": s.exit_code,
                    })
                }).collect();
                Ok(json!({"sessions": list}))
            }
            "log" => {
                let id = req.session_id.ok_or_else(|| anyhow!("session_id required"))?;
                let sessions = self.manager.sessions.read().await;
                let session = sessions.get(&id).ok_or_else(|| anyhow!("session not found"))?;
                let limit = req.limit.unwrap_or(200);
                let stdout_lines: Vec<&str> = session.stdout.lines().rev().take(limit).collect();
                Ok(json!({
                    "stdout": stdout_lines.into_iter().rev().collect::<Vec<_>>().join("\n"),
                    "stderr": session.stderr.clone(),
                    "status": match session.status {
                        ProcessStatus::Running => "running",
                        ProcessStatus::Completed => "completed",
                        ProcessStatus::Failed => "failed",
                    },
                    "exit_code": session.exit_code,
                }))
            }
            "kill" => {
                let id = req.session_id.ok_or_else(|| anyhow!("session_id required"))?;
                let mut sessions = self.manager.sessions.write().await;
                sessions.remove(&id).ok_or_else(|| anyhow!("session not found"))?;
                Ok(json!({"killed": id}))
            }
            _ => Err(anyhow!("unsupported process action: {}", req.action)),
        }
    }
}

/// Spawn a background process and track it in the manager.
pub async fn spawn_background(
    manager: &ProcessManager,
    command: &str,
    workdir: &std::path::Path,
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    let session = ProcessSession {
        id: id.clone(),
        command: command.to_string(),
        started_at: chrono::Utc::now(),
        status: ProcessStatus::Running,
        stdout: String::new(),
        stderr: String::new(),
        exit_code: None,
    };
    manager.sessions.write().await.insert(id.clone(), session);

    let sessions = manager.sessions.clone();
    let cmd = command.to_string();
    let wd = workdir.to_path_buf();
    let sid = id.clone();

    tokio::spawn(async move {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&wd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await;

        let mut sessions = sessions.write().await;
        if let Some(session) = sessions.get_mut(&sid) {
            match output {
                Ok(out) => {
                    session.stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    session.stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    session.exit_code = out.status.code();
                    session.status = if out.status.success() {
                        ProcessStatus::Completed
                    } else {
                        ProcessStatus::Failed
                    };
                }
                Err(e) => {
                    session.stderr = e.to_string();
                    session.status = ProcessStatus::Failed;
                }
            }
        }
    });

    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn list_empty() {
        let tool = ProcessTool::new();
        let out = tool.run(json!({"action": "list"})).await.unwrap();
        assert_eq!(out["sessions"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn spawn_and_track() {
        let mgr = ProcessManager::new();
        let tmp = tempfile::TempDir::new().unwrap();
        let id = spawn_background(&mgr, "echo done", tmp.path()).await.unwrap();
        // Wait for completion
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let sessions = mgr.sessions.read().await;
        let s = sessions.get(&id).unwrap();
        assert_eq!(s.status, ProcessStatus::Completed);
        assert!(s.stdout.contains("done"));
    }
}
