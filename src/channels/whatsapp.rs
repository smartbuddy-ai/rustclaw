use crate::auth;
use crate::config::{Config, WhatsAppConfig, resolve_secret};
use crate::workspace;
use anyhow::Result;
use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
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
    if q.mode.as_deref() == Some("subscribe")
        && q.verify_token.as_deref() == Some(expected)
    {
        Ok(q.challenge.unwrap_or_default())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn handle_webhook(
    State(state): State<Arc<AppState>>,
    axum::Json(payload): axum::Json<WebhookPayload>,
) -> StatusCode {
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

                tracing::info!(from = %msg.from, text = %text, "whatsapp message");

                let system = workspace::build_system_prompt(&state.cfg).unwrap_or_default();
                let messages = vec![auth::ChatMessage {
                    role: "user".into(),
                    content: text,
                }];

                let reply = match auth::complete(&state.cfg, &messages, Some(&system)).await {
                    Ok(r) => r.content,
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
    let phone_id = wa.phone_number_id.as_deref()
        .ok_or_else(|| anyhow::anyhow!("WhatsApp phone_number_id not configured"))?;
    let api_url = wa.api_url.as_deref()
        .unwrap_or("https://graph.facebook.com/v21.0");

    let body = SendTextBody {
        messaging_product: "whatsapp".into(),
        to: to.into(),
        msg_type: "text".into(),
        text: TextBody { body: text.into() },
    };

    client
        .post(format!("{api_url}/{phone_id}/messages"))
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?;

    Ok(())
}

/// Run WhatsApp webhook server.
pub async fn run(cfg: &Config, wa: &WhatsAppConfig) -> Result<()> {
    let token = resolve_secret(&wa.access_token, "WHATSAPP_ACCESS_TOKEN")
        .ok_or_else(|| anyhow::anyhow!("WhatsApp access token not configured"))?;

    let state = Arc::new(AppState {
        cfg: cfg.clone(),
        wa: wa.clone(),
        token,
        client: reqwest::Client::new(),
    });

    let app = Router::new()
        .route("/webhook", get(verify_webhook))
        .route("/webhook", post(handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", wa.webhook_port);
    tracing::info!(channel = "whatsapp", addr = %addr, "starting webhook server");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
