pub mod context_guard;
pub mod loop_detector;

use crate::config::Config;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    pub id: String,
    pub model: String,
    pub identity: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Default, Clone)]
pub struct AgentRegistry {
    agents: HashMap<String, Agent>,
    channel_bindings: HashMap<String, String>,
}

impl AgentRegistry {
    pub fn load_from_config(cfg: &Config) -> Self {
        let mut s = Self::default();
        let default = Agent { id: "default".into(), model: cfg.auth.default_model.clone(), identity: "rustclaw".into(), config: serde_json::json!({}) };
        s.agents.insert(default.id.clone(), default);
        s
    }
    pub fn list_agents(&self) -> Vec<Agent> { self.agents.values().cloned().collect() }
    pub fn bind_channel(&mut self, channel: &str, agent_id: &str) { self.channel_bindings.insert(channel.into(), agent_id.into()); }
    pub fn route_for_channel(&self, channel: &str) -> Option<&Agent> {
        self.channel_bindings.get(channel).and_then(|id| self.agents.get(id)).or_else(|| self.agents.get("default"))
    }
    pub fn spawn_subagent<F>(&self, agent_id: &str, fut: F) -> Result<tokio::task::JoinHandle<()>>
    where F: std::future::Future<Output = ()> + Send + 'static {
        if !self.agents.contains_key(agent_id) { anyhow::bail!("unknown agent"); }
        Ok(tokio::spawn(fut))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, ChannelsConfig, CronConfig, GatewayConfig, MemoryConfig, NodeConfig, TunnelConfig};
    #[test]
    fn registry_routes() {
        let cfg = Config { workspace_dir: std::path::PathBuf::from("."), auth: AuthConfig::default(), channels: ChannelsConfig::default(), cron: CronConfig::default(), node: NodeConfig::default(), memory: MemoryConfig::default(), tunnel: TunnelConfig::default(), gateway: GatewayConfig::default(), tools: crate::config::ToolsConfig::default() };
        let mut r = AgentRegistry::load_from_config(&cfg);
        r.bind_channel("telegram", "default");
        assert_eq!(r.route_for_channel("telegram").unwrap().id, "default");
    }
    #[tokio::test]
    async fn spawn_subagent_works() {
        let cfg = Config { workspace_dir: std::path::PathBuf::from("."), auth: AuthConfig::default(), channels: ChannelsConfig::default(), cron: CronConfig::default(), node: NodeConfig::default(), memory: MemoryConfig::default(), tunnel: TunnelConfig::default(), gateway: GatewayConfig::default(), tools: crate::config::ToolsConfig::default() };
        let r = AgentRegistry::load_from_config(&cfg);
        let h = r.spawn_subagent("default", async {}).unwrap();
        h.await.unwrap();
    }
}
