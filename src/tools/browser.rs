use crate::tools::Tool;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use futures::{SinkExt, StreamExt};
use std::sync::atomic::{AtomicU64, Ordering};

static CDP_MSG_ID: AtomicU64 = AtomicU64::new(1);

pub struct BrowserTool;

#[derive(Deserialize)]
struct BrowserReq {
    action: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    output_path: Option<String>,
    #[serde(default)]
    selector: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    expression: Option<String>,
    /// CDP endpoint (ws://...), auto-detected if not provided.
    #[serde(default)]
    cdp_endpoint: Option<String>,
}

impl BrowserTool {
    /// Detect CDP endpoint from common locations.
    fn detect_cdp_endpoint(req: &BrowserReq) -> Option<String> {
        if let Some(ep) = &req.cdp_endpoint {
            return Some(ep.clone());
        }
        // Try common Chrome DevTools ports
        std::env::var("CDP_ENDPOINT").ok()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn name(&self) -> &'static str { "browser" }

    async fn run(&self, input: Value) -> Result<Value> {
        let req: BrowserReq = serde_json::from_value(input)?;
        match req.action.as_str() {
            "open" | "navigate" => {
                let url = req.url.as_deref().ok_or_else(|| anyhow!("url required"))?;
                if let Some(cdp) = Self::detect_cdp_endpoint(&req) {
                    return cdp_navigate(&cdp, url).await;
                }
                // Fallback: HTTP fetch
                let html = reqwest::get(url).await?.text().await?;
                Ok(json!({"url": url, "bytes": html.len(), "method": "http_fetch"}))
            }
            "screenshot" => {
                let url = req.url.as_deref().ok_or_else(|| anyhow!("url required"))?;
                let target = req.output_path.as_deref().ok_or_else(|| anyhow!("output_path required"))?;
                if let Some(cdp) = Self::detect_cdp_endpoint(&req) {
                    return cdp_screenshot(&cdp, url, target).await;
                }
                // Fallback: thum.io
                let shot_url = format!("https://image.thum.io/get/png/noanimate/{}", url);
                let bytes = reqwest::get(shot_url).await?.bytes().await?;
                std::fs::write(target, &bytes)?;
                Ok(json!({"saved": target, "bytes": bytes.len(), "method": "thum_io"}))
            }
            "snapshot" => {
                let cdp = Self::detect_cdp_endpoint(&req).ok_or_else(|| anyhow!("CDP endpoint required for snapshot"))?;
                cdp_snapshot(&cdp).await
            }
            "evaluate" => {
                let cdp = Self::detect_cdp_endpoint(&req).ok_or_else(|| anyhow!("CDP endpoint required for evaluate"))?;
                let expr = req.expression.as_deref().ok_or_else(|| anyhow!("expression required"))?;
                cdp_evaluate(&cdp, expr).await
            }
            "click" => {
                let cdp = Self::detect_cdp_endpoint(&req).ok_or_else(|| anyhow!("CDP endpoint required for click"))?;
                let selector = req.selector.as_deref().ok_or_else(|| anyhow!("selector required"))?;
                cdp_click(&cdp, selector).await
            }
            "type" => {
                let cdp = Self::detect_cdp_endpoint(&req).ok_or_else(|| anyhow!("CDP endpoint required for type"))?;
                let selector = req.selector.as_deref().ok_or_else(|| anyhow!("selector required"))?;
                let text = req.text.as_deref().ok_or_else(|| anyhow!("text required"))?;
                cdp_type(&cdp, selector, text).await
            }
            _ => Err(anyhow!("unsupported browser action: {}", req.action)),
        }
    }
}

/// Send a CDP command and get the response.
async fn cdp_send(endpoint: &str, method: &str, params: Value) -> Result<Value> {
    let (mut ws, _) = tokio_tungstenite::connect_async(endpoint).await
        .map_err(|e| anyhow!("CDP connection failed: {e}"))?;

    let id = CDP_MSG_ID.fetch_add(1, Ordering::Relaxed);
    let msg = json!({
        "id": id,
        "method": method,
        "params": params,
    });

    ws.send(WsMessage::Text(msg.to_string().into())).await?;

    // Read until we get our response
    let timeout = tokio::time::Duration::from_secs(10);
    let result = tokio::time::timeout(timeout, async {
        while let Some(msg) = ws.next().await {
            match msg {
                Ok(WsMessage::Text(text)) => {
                    if let Ok(val) = serde_json::from_str::<Value>(&text) {
                        if val.get("id").and_then(|i| i.as_u64()) == Some(id) {
                            return Ok(val);
                        }
                    }
                }
                Ok(_) => continue,
                Err(e) => return Err(anyhow!("CDP ws error: {e}")),
            }
        }
        Err(anyhow!("CDP connection closed"))
    }).await.map_err(|_| anyhow!("CDP response timeout"))??;

    let _ = ws.close(None).await;
    Ok(result)
}

async fn cdp_navigate(endpoint: &str, url: &str) -> Result<Value> {
    let result = cdp_send(endpoint, "Page.navigate", json!({"url": url})).await?;
    Ok(json!({
        "url": url,
        "frame_id": result.get("result").and_then(|r| r.get("frameId")),
        "method": "cdp",
    }))
}

async fn cdp_screenshot(endpoint: &str, url: &str, output_path: &str) -> Result<Value> {
    // Navigate first
    cdp_send(endpoint, "Page.navigate", json!({"url": url})).await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let result = cdp_send(endpoint, "Page.captureScreenshot", json!({"format": "png"})).await?;
    let data = result.get("result")
        .and_then(|r| r.get("data"))
        .and_then(|d| d.as_str())
        .ok_or_else(|| anyhow!("no screenshot data"))?;

    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)?;
    std::fs::write(output_path, &bytes)?;
    Ok(json!({"saved": output_path, "bytes": bytes.len(), "method": "cdp"}))
}

async fn cdp_snapshot(endpoint: &str) -> Result<Value> {
    let result = cdp_send(endpoint, "Runtime.evaluate", json!({
        "expression": "document.body.innerText",
        "returnByValue": true,
    })).await?;
    let text = result.get("result")
        .and_then(|r| r.get("result"))
        .and_then(|r| r.get("value"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    Ok(json!({"content": text, "method": "cdp"}))
}

async fn cdp_evaluate(endpoint: &str, expression: &str) -> Result<Value> {
    let result = cdp_send(endpoint, "Runtime.evaluate", json!({
        "expression": expression,
        "returnByValue": true,
    })).await?;
    Ok(json!({
        "result": result.get("result").and_then(|r| r.get("result")),
        "method": "cdp",
    }))
}

async fn cdp_click(endpoint: &str, selector: &str) -> Result<Value> {
    cdp_send(endpoint, "Runtime.evaluate", json!({
        "expression": format!("document.querySelector('{}').click()", selector.replace('\'', "\\'")),
    })).await?;
    Ok(json!({"clicked": selector, "method": "cdp"}))
}

async fn cdp_type(endpoint: &str, selector: &str, text: &str) -> Result<Value> {
    // Focus the element
    cdp_send(endpoint, "Runtime.evaluate", json!({
        "expression": format!("document.querySelector('{}').focus()", selector.replace('\'', "\\'")),
    })).await?;

    // Type each character
    for ch in text.chars() {
        cdp_send(endpoint, "Input.dispatchKeyEvent", json!({
            "type": "keyDown",
            "text": ch.to_string(),
        })).await?;
        cdp_send(endpoint, "Input.dispatchKeyEvent", json!({
            "type": "keyUp",
            "text": ch.to_string(),
        })).await?;
    }

    Ok(json!({"typed": text, "selector": selector, "method": "cdp"}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn open_url_fallback_works() {
        let out = BrowserTool.run(json!({"action":"open","url":"https://example.com"})).await.unwrap();
        assert!(out["bytes"].as_u64().unwrap() > 0);
        assert_eq!(out["method"], "http_fetch");
    }

    #[tokio::test]
    async fn unsupported_action_errors() {
        let result = BrowserTool.run(json!({"action":"fly"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn navigate_requires_url() {
        let result = BrowserTool.run(json!({"action":"navigate"})).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn snapshot_requires_cdp() {
        let result = BrowserTool.run(json!({"action":"snapshot"})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("CDP"));
    }
}
