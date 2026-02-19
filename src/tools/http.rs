use crate::guardd::{Action, GuardConfig, GuardDaemon, Verdict};
use crate::guardd::policy::{Rule, RuleConditions};
use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;

pub struct HttpTool;

#[derive(Deserialize)]
struct HttpReq {
    method: String,
    url: String,
    headers: Option<HashMap<String, String>>,
    body: Option<Value>,
    bearer_token: Option<String>,
}

#[async_trait]
impl Tool for HttpTool {
    fn name(&self) -> &'static str { "http" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: HttpReq = serde_json::from_value(input)?;
        let mut cfg = GuardConfig::default();
        cfg.policy.rules.push(Rule { action_pattern: "api:*:*".into(), verdict: Verdict::Allow, conditions: RuleConditions::default() });
        let guard = GuardDaemon::new(cfg)?;
        let verdict = guard.authorize(&Action::ApiCall { method: req.method.clone(), endpoint: req.url.clone() });
        if verdict != Verdict::Allow { return Err(anyhow!("api call blocked by guardd")); }

        let client = reqwest::Client::new();
        let method: Method = req.method.parse()?;
        let mut builder = client.request(method, &req.url);
        if let Some(h) = req.headers { for (k,v) in h { builder = builder.header(k, v); } }
        if let Some(t) = req.bearer_token { builder = builder.bearer_auth(t); }
        if let Some(body) = req.body { builder = builder.json(&body); }
        let resp = builder.send().await?;
        let status = resp.status().as_u16();
        let text = resp.text().await?;
        Ok(json!({"status": status, "body": text}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn http_get_works() {
        let out = HttpTool.run(json!({"method":"GET","url":"https://example.com"})).await.unwrap();
        assert!(out["status"].as_u64().unwrap() >= 200);
    }
}
