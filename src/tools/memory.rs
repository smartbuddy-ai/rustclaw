use crate::memory::SqliteMemory;
use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};

pub struct MemoryTool {
    pub memory: SqliteMemory,
}

fn simple_embed(s: &str) -> Vec<f32> {
    let mut v = vec![0.0_f32; 32];
    for (i, b) in s.as_bytes().iter().enumerate() {
        v[i % 32] += *b as f32 / 255.0;
    }
    v
}

#[derive(Deserialize)]
struct Req {
    action: String,
    key: Option<String>,
    content: Option<String>,
    category: Option<String>,
    query: Option<String>,
    limit: Option<usize>,
    mode: Option<String>,
}

#[async_trait]
impl Tool for MemoryTool {
    fn name(&self) -> &'static str { "memory" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: Req = serde_json::from_value(input)?;
        match req.action.as_str() {
            "store" => {
                self.memory.upsert(req.key.as_deref().ok_or_else(|| anyhow!("key required"))?, req.content.as_deref().unwrap_or(""), req.category.as_deref().unwrap_or("core"))?;
                Ok(json!({"ok": true}))
            }
            "recall" => {
                let r = self.memory.get(req.key.as_deref().ok_or_else(|| anyhow!("key required"))?)?;
                Ok(json!({"record": r}))
            }
            "search" => {
                let query = req.query.as_deref().unwrap_or("");
                if req.mode.as_deref() == Some("semantic") {
                    let db_path = self.memory.db_path();
                    let store = crate::memory::vector::VectorStore::open(&db_path.with_file_name("vectors.db"))?;
                    let emb = if let Ok(key) = std::env::var("OPENAI_API_KEY") {
                        crate::memory::vector::embed_text_openai(&key, query).await?
                    } else {
                        simple_embed(query)
                    };
                    let hits = store.semantic_search(&emb, req.limit.unwrap_or(10))?;
                    Ok(json!({"records": hits, "mode": "semantic"}))
                } else {
                    let rows = self.memory.search(query, req.limit.unwrap_or(10))?;
                    Ok(json!({"records": rows, "mode": "keyword"}))
                }
            }
            "forget" => {
                let key = req.key.ok_or_else(|| anyhow!("key required"))?;
                let ok = self.memory.forget(&key)?;
                Ok(json!({"ok": ok}))
            }
            _ => Err(anyhow!("unsupported action")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, ChannelsConfig, Config, CronConfig, MemoryConfig, NodeConfig, TunnelConfig};
    use serde_json::json;
    use tempfile::TempDir;

    #[tokio::test]
    async fn memory_store_recall_search_forget() {
        let tmp = TempDir::new().unwrap();
        let cfg = Config { workspace_dir: tmp.path().to_path_buf(), auth: AuthConfig::default(), channels: ChannelsConfig::default(), cron: CronConfig::default(), node: NodeConfig::default(), memory: MemoryConfig::default(), tunnel: TunnelConfig::default(), gateway: crate::config::GatewayConfig::default(), tools: crate::config::ToolsConfig::default() };
        let tool = MemoryTool { memory: SqliteMemory::from_config(&cfg).unwrap() };
        tool.run(json!({"action":"store","key":"k1","content":"v1"})).await.unwrap();
        let out = tool.run(json!({"action":"recall","key":"k1"})).await.unwrap();
        assert_eq!(out["record"]["content"], "v1");
        let out = tool.run(json!({"action":"search","query":"v1"})).await.unwrap();
        assert!(!out["records"].as_array().unwrap().is_empty());
        let out = tool.run(json!({"action":"forget","key":"k1"})).await.unwrap();
        assert_eq!(out["ok"], true);
    }
}
