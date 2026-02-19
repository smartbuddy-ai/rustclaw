use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use std::path::PathBuf;
use tokio::process::Command;

pub struct GitTool {
    repo: PathBuf,
}

impl GitTool { pub fn new(repo: PathBuf) -> Self { Self { repo } } }

#[derive(Deserialize)]
struct GitReq {
    action: String,
    message: Option<String>,
    remote: Option<String>,
    branch: Option<String>,
}

#[async_trait]
impl Tool for GitTool {
    fn name(&self) -> &'static str { "git" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: GitReq = serde_json::from_value(input)?;
        let args: Vec<String> = match req.action.as_str() {
            "status" => vec!["status".into(), "--short".into()],
            "diff" => vec!["diff".into()],
            "log" => vec!["log".into(), "--oneline".into(), "-n".into(), "20".into()],
            "pull" => vec!["pull".into(), req.remote.unwrap_or("origin".into()), req.branch.unwrap_or("main".into())],
            "push" => vec!["push".into(), req.remote.unwrap_or("origin".into()), req.branch.unwrap_or("main".into())],
            "commit" => {
                let msg = req.message.ok_or_else(|| anyhow!("commit message required"))?;
                vec!["commit".into(), "-m".into(), msg]
            }
            _ => return Err(anyhow!("unsupported git action")),
        };
        let output = Command::new("git").args(args).current_dir(&self.repo).output().await?;
        Ok(json!({
            "stdout": String::from_utf8_lossy(&output.stdout),
            "stderr": String::from_utf8_lossy(&output.stderr),
            "code": output.status.code().unwrap_or(-1)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn git_status_runs() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let out = GitTool::new(repo).run(json!({"action":"status"})).await.unwrap();
        assert!(out["code"].as_i64().unwrap() == 0);
    }
}
