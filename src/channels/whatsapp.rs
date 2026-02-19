use crate::config::{Config, WhatsAppConfig};
use crate::guardd::channel_auth::verify_whatsapp_signature;
use anyhow::Result;
use axum::{
    Router,
    body::Bytes,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone)]
struct AppState {
    cfg: Config,
    wa: WhatsAppConfig,
    token: String,
    client: reqwest::Client,
}

/// WhatsApp Cloud API webhook verification.
#[derive(Deserialize)]
struct VerifyQuery {
    #[serde(rename = "hub.mode")]
    mode: Option<String>,
    #[serde(rename = "hub.verify_token")]
    verify_token: Option<String>,
    #[serde(rename = "hub.challenge")]
    challenge: Option<String>,
}

/// WhatsApp Cloud API webhook payload.
#[derive(Deserialize)]
struct WebhookPayload {
    entry: Option<Vec<Entry>>,
}

#[derive(Deserialize)]
struct Entry {
    changes: Option<Vec<Change>>,
}

#[derive(Deserialize)]
struct Change {
    value: Option<ChangeValue>,
}

#[derive(Deserialize)]
struct ChangeValue {
    messages: Option<Vec<WaMessage>>,
}

#[derive(Deserialize)]
struct WaMessage {
    from: String,
    id: String,
    #[serde(rename = "type")]
    msg_type: String,
    text: Option<WaText>,
    timestamp: String,
}

#[derive(Deserialize)]
struct WaText {
    body: String,
}

#[derive(Serialize)]
struct SendTextBody {
    messaging_product: String,
    to: String,
    #[serde(rename = "type")]
    msg_type: String,
    text: TextBody,
}

#[derive(Serialize)]
struct TextBody {
    body: String,
}

async fn verify_webhook(
    State(state): State<Arc<AppState>>,
    Query(q): Query<VerifyQuery>,
) -> Result<String, StatusCode> {
    let expected = state.wa.verify_token.as_deref().unwrap_or("rustclaw");
    if q.mode.as_deref() == Some("subscribe") && q.verify_token.as_deref() == Some(expected) {
        Ok(q.challenge.unwrap_or_default())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

fn verify_signature(headers: &HeaderMap, body: &str, wa: &WhatsAppConfig) -> bool {
    let Some(app_secret) = wa.app_secret.as_deref() else {
        return true;
    };

    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let timestamp = headers
        .get("x-timestamp")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    verify_whatsapp_signature(app_secret, timestamp, body, signature).unwrap_or(false)
}

async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> StatusCode {
    let raw = String::from_utf8_lossy(&body).to_string();
    if !verify_signature(&headers, &raw, &state.wa) {
        return StatusCode::UNAUTHORIZED;
    }

    let payload: WebhookPayload = match serde_json::from_slice(&body) {
        Ok(p) => p,
        Err(_) => return StatusCode::BAD_REQUEST,
    };

    let entries = match payload.entry {
        Some(e) => e,
        None => return StatusCode::OK,
    };

    for entry in entries {
        let changes = entry.changes.unwrap_or_default();
        for change in changes {
            let messages = change.value.and_then(|v| v.messages).unwrap_or_default();
            for msg in messages {
                if msg.msg_type != "text" {
                    continue;
                }
                let text = match msg.text {
                    Some(ref t) => t.body.clone(),
                    None => continue,
                };

                tracing::info!(from = %msg.from, id = %msg.id, ts = %msg.timestamp, text = %text, "whatsapp message");

                let session_id = format!("whatsapp:{}", msg.from);
                let reply = match crate::chat::send_with_session(&state.cfg, &session_id, &text).await {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::error!(error = %e, "LLM error");
                        "⚠️ Error processing your message.".into()
                    }
                };

                if let Err(e) = send_wa_message(&state.client, &state.wa, &state.token, &msg.from, &reply).await {
                    tracing::error!(error = %e, "failed to send WhatsApp reply");
                }
            }
        }
    }

    StatusCode::OK
}

async fn send_wa_message(
    client: &reqwest::Client,
    wa: &WhatsAppConfig,
    token: &str,
    to: &str,
    text: &str,
) -> Result<()> {
    let phone_id = wa
        .phone_number_id
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("WhatsApp phone_number_id not configured"))?;
    let api_url = wa.api_url.as_deref().unwrap_or("https://graph.facebook.com/v21.0");

    let body = SendTextBody {
        messaging_product: "whatsapp".into(),
        to: to.into(),
        msg_type: "text".into(),
        text: TextBody { body: text.into() },
    };

    let resp = client
        .post(format!("{api_url}/{phone_id}/messages"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("WhatsApp send failed with {}", resp.status());
    }

    Ok(())
}

/// Run WhatsApp webhook server.
pub async fn run(cfg: &Config, wa: &WhatsAppConfig) -> Result<()> {
    let token = wa
        .access_token
        .clone()
        .or_else(|| std::env::var("WHATSAPP_ACCESS_TOKEN").ok())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WHATSAPP_ACCESS_TOKEN not set. Run `rustclaw init` or set the env var."))?;

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        wa: wa.clone(),
        token,
        client: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/webhook/whatsapp", get(verify_webhook))
        .route("/webhook/whatsapp", post(handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", wa.webhook_port);
    tracing::info!(channel = "whatsapp", addr = %addr, "starting webhook server");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_signature_disabled_without_secret() {
        let wa = WhatsAppConfig {
            enabled: true,
            api_url: None,
            access_token: None,
            verify_token: None,
            phone_number_id: None,
            webhook_port: 8090,
            app_secret: None,
        };
        assert!(verify_signature(&HeaderMap::new(), "{}", &wa));
    }
}
