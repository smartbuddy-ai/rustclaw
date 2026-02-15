use crate::auth;
use crate::config::{Config, SlackConfig};
use crate::workspace;
use anyhow::Result;
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    cfg: Config,
    #[allow(dead_code)]
    sl: SlackConfig,
    bot_token: String,
    client: reqwest::Client,
}

/// Slack Events API payload.
#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(tag = "type")]
enum SlackEvent {
    #[serde(rename = "url_verification")]
    UrlVerification { challenge: String },
    #[serde(rename = "event_callback")]
    EventCallback { event: EventPayload },
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct EventPayload {
    #[serde(rename = "type")]
    event_type: String,
    text: Option<String>,
    user: Option<String>,
    channel: Option<String>,
    ts: Option<String>,
    thread_ts: Option<String>,
    bot_id: Option<String>,
}

#[derive(Serialize)]
struct PostMessageBody {
    channel: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_ts: Option<String>,
}

async fn handle_events(
    State(state): State<Arc<AppState>>,
    axum::Json(event): axum::Json<serde_json::Value>,
) -> Result<axum::Json<serde_json::Value>, StatusCode> {
    // Handle url_verification
    if event.get("type").and_then(|t| t.as_str()) == Some("url_verification") {
        let challenge = event.get("challenge").and_then(|c| c.as_str()).unwrap_or("");
        return Ok(axum::Json(serde_json::json!({ "challenge": challenge })));
    }

    // Handle event_callback
    if event.get("type").and_then(|t| t.as_str()) == Some("event_callback") {
        if let Some(evt) = event.get("event") {
            let event_type = evt.get("type").and_then(|t| t.as_str()).unwrap_or("");

            // Skip bot messages
            if evt.get("bot_id").is_some() {
                return Ok(axum::Json(serde_json::json!({"ok": true})));
            }

            if event_type == "message" || event_type == "app_mention" {
                let text = evt.get("text").and_then(|t| t.as_str()).unwrap_or("");
                let channel = evt.get("channel").and_then(|c| c.as_str()).unwrap_or("");
                let thread_ts = evt.get("thread_ts").or(evt.get("ts"))
                    .and_then(|t| t.as_str())
                    .map(String::from);

                if !text.is_empty() && !channel.is_empty() {
                    tracing::info!(channel = channel, text = text, "slack message");

                    let system = workspace::build_system_prompt(&state.cfg).unwrap_or_default();
                    let messages = vec![auth::ChatMessage {
                        role: "user".into(),
                        content: text.to_string(),
                    }];

                    let reply = match auth::complete(&state.cfg, &messages, Some(&system)).await {
                        Ok(r) => r.content,
                        Err(e) => {
                            tracing::error!(error = %e, "LLM error");
                            "⚠️ Error processing your message.".into()
                        }
                    };

                    if let Err(e) = send_slack_message(
                        &state.client,
                        &state.bot_token,
                        channel,
                        &reply,
                        thread_ts,
                    ).await {
                        tracing::error!(error = %e, "failed to send Slack reply");
                    }
                }
            }
        }
    }

    Ok(axum::Json(serde_json::json!({"ok": true})))
}

async fn send_slack_message(
    client: &reqwest::Client,
    token: &str,
    channel: &str,
    text: &str,
    thread_ts: Option<String>,
) -> Result<()> {
    let body = PostMessageBody {
        channel: channel.into(),
        text: text.into(),
        thread_ts,
    };

    let resp = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Slack API error: {text}");
    }

    Ok(())
}

/// Run Slack events HTTP server (Events API mode).
pub async fn run(cfg: &Config, sl: &SlackConfig) -> Result<()> {
    let bot_token = sl
        .bot_token
        .clone()
        .or_else(|| std::env::var("SLACK_BOT_TOKEN").ok())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("SLACK_BOT_TOKEN not set. Run `rustclaw init` or set the env var."))?;

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        sl: sl.clone(),
        bot_token,
        client: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/slack/events", post(handle_events))
        .with_state(state);

    let addr = "0.0.0.0:8091";
    tracing::info!(channel = "slack", addr = %addr, "starting events server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
