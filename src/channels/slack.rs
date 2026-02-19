use crate::config::{Config, SlackConfig};
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

fn verify_slack_request(headers: &axum::http::HeaderMap, body: &str, sl: &SlackConfig) -> bool {
    let Some(signing_secret) = sl.signing_secret.as_deref() else {
        return true; // No secret configured = skip verification
    };

    let timestamp = headers
        .get("x-slack-request-timestamp")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let signature = headers
        .get("x-slack-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    crate::guardd::channel_auth::verify_slack_signature(signing_secret, timestamp, body, signature)
        .unwrap_or(false)
}

async fn handle_events_raw(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> Result<axum::Json<serde_json::Value>, StatusCode> {
    let raw = String::from_utf8_lossy(&body).to_string();
    if !verify_slack_request(&headers, &raw, &state.sl) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let event: serde_json::Value = serde_json::from_slice(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    handle_events_inner(state, event).await
}

async fn handle_events_inner(
    state: Arc<AppState>,
    event: serde_json::Value,
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

                    let session_id = format!("slack:{}", channel);
                    let reply = match crate::chat::send_with_session(&state.cfg, &session_id, text).await {
                        Ok(r) => r,
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
        .route("/slack/events", post(handle_events_raw))
        .with_state(state);

    let addr = "0.0.0.0:8091";
    tracing::info!(channel = "slack", addr = %addr, "starting events server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
