use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use crate::config::Config;

pub mod browser;
pub mod file;
pub mod git;
pub mod http;
pub mod image_info;
pub mod memory;
pub mod process;
pub mod shell;
pub mod web;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    async fn run(&self, input: Value) -> Result<Value>;
}

#[derive(Default)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_defaults(cfg: &Config) -> anyhow::Result<Self> {
        let mut reg = Self::new();
        reg.register(file::FileTool::new(cfg.workspace_dir.clone()));
        reg.register(http::HttpTool);
        reg.register(git::GitTool::new(cfg.workspace_dir.clone()));
        reg.register(browser::BrowserTool);
        reg.register(image_info::ImageInfoTool);
        reg.register(web::WebSearchTool::new(&cfg.tools.web));
        reg.register(web::WebFetchTool::new(&cfg.tools.web));
        reg.register(memory::MemoryTool { memory: crate::memory::SqliteMemory::from_config(cfg)? });
        reg.register(shell::ShellTool::new(cfg.workspace_dir.clone()));
        reg.register(process::ProcessTool::new());
        Ok(reg)
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
    }

    pub async fn execute(&self, name: &str, input: Value) -> Result<Value> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow!("unknown tool: {name}"))?;
        tool.run(input).await
    }

    pub fn names(&self) -> Vec<String> {
        let mut n: Vec<String> = self.tools.keys().cloned().collect();
        n.sort();
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Echo;

    #[async_trait]
    impl Tool for Echo {
        fn name(&self) -> &'static str { "echo" }
        async fn run(&self, input: Value) -> Result<Value> { Ok(input) }
    }

    #[tokio::test]
    async fn registry_executes_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Echo);
        let out = reg.execute("echo", json!({"x":1})).await.unwrap();
        assert_eq!(out["x"], 1);
    }
}
