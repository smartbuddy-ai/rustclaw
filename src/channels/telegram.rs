use crate::auth;
use crate::config::{Config, TelegramConfig};
use crate::workspace;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Resolve the bot token from config or environment (.env loaded by dotenvy).
fn resolve_token(tg: &TelegramConfig) -> Result<String> {
    tg.bot_token
        .clone()
        .or_else(|| std::env::var("TELEGRAM_BOT_TOKEN").ok())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("TELEGRAM_BOT_TOKEN not set. Run `rustclaw init` or set the env var."))
}

/// Telegram Bot API response wrapper.
#[derive(Deserialize)]
struct TgResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Deserialize)]
struct Update {
    update_id: i64,
    message: Option<Message>,
}

#[derive(Deserialize)]
struct Message {
    message_id: i64,
    from: Option<User>,
    chat: Chat,
    text: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    reply_to_message: Option<Box<Message>>,
    message_thread_id: Option<i64>,
}

#[derive(Deserialize)]
struct User {
    id: i64,
    first_name: String,
    last_name: Option<String>,
    #[allow(dead_code)]
    username: Option<String>,
}

#[derive(Deserialize)]
struct Chat {
    id: i64,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    chat_type: String,
}

#[derive(Serialize)]
struct SendMessageBody {
    chat_id: i64,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_to_message_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message_thread_id: Option<i64>,
}

/// Check if sender is allowed.
fn is_allowed(tg: &TelegramConfig, sender_id: i64) -> bool {
    if tg.allow_from.is_empty() {
        return true; // No allowlist = allow all
    }
    tg.allow_from.iter().any(|id| {
        id.parse::<i64>().ok() == Some(sender_id)
    })
}

/// Send a message via Telegram Bot API.
async fn send_message(
    client: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
    reply_to: Option<i64>,
    thread_id: Option<i64>,
) -> Result<()> {
    // Chunk text if > 4096 chars
    let chunks = chunk_text(text, 4000);
    for chunk in chunks {
        let body = SendMessageBody {
            chat_id,
            text: chunk,
            parse_mode: Some("Markdown".into()),
            reply_to_message_id: reply_to,
            message_thread_id: thread_id,
        };

        let resp = client
            .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            // Retry without parse_mode on markdown errors
            let body_plain = SendMessageBody {
                chat_id,
                text: body.text,
                parse_mode: None,
                reply_to_message_id: reply_to,
                message_thread_id: thread_id,
            };
            client
                .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
                .json(&body_plain)
                .send()
                .await?;
        }
    }
    Ok(())
}

fn chunk_text(text: &str, limit: usize) -> Vec<String> {
    if text.len() <= limit {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.len() <= limit {
            chunks.push(remaining.to_string());
            break;
        }
        // Try to split at a newline
        let split_at = remaining[..limit]
            .rfind('\n')
            .unwrap_or(limit);
        chunks.push(remaining[..split_at].to_string());
        remaining = &remaining[split_at..].trim_start_matches('\n');
    }
    chunks
}

/// Run Telegram long-polling loop.
pub async fn run(cfg: &Config, tg: &TelegramConfig) -> Result<()> {
    let token = resolve_token(tg)?;
    let client = reqwest::Client::new();
    let mut offset: Option<i64> = None;

    tracing::info!(channel = "telegram", "starting polling");

    loop {
        let mut url = format!(
            "https://api.telegram.org/bot{token}/getUpdates?timeout=30"
        );
        if let Some(off) = offset {
            url.push_str(&format!("&offset={off}"));
        }

        let resp = client.get(&url).send().await;
        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "polling error, retrying in 5s");
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        let data: TgResponse<Vec<Update>> = match resp.json().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(error = %e, "parse error, retrying");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
        };

        if !data.ok {
            tracing::error!(desc = ?data.description, "Telegram API error");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }

        let updates = data.result.unwrap_or_default();
        for update in updates {
            offset = Some(update.update_id + 1);

            let msg = match update.message {
                Some(m) => m,
                None => continue,
            };

            let text = match msg.text {
                Some(ref t) => t.clone(),
                None => continue,
            };

            let sender_id = msg.from.as_ref().map(|u| u.id).unwrap_or(0);
            if !is_allowed(tg, sender_id) {
                tracing::debug!(sender_id, "ignoring message from non-allowed sender");
                continue;
            }

            let sender_name = msg.from.as_ref().map(|u| {
                let mut name = u.first_name.clone();
                if let Some(ref last) = u.last_name {
                    name.push(' ');
                    name.push_str(last);
                }
                name
            });

            tracing::info!(
                chat_id = msg.chat.id,
                sender = ?sender_name,
                text = %text,
                "received message"
            );

            // Build system prompt from workspace
            let system = workspace::build_system_prompt(cfg).unwrap_or_default();

            // Simple single-turn completion
            let messages = vec![auth::ChatMessage {
                role: "user".into(),
                content: text,
            }];

            match auth::complete(cfg, &messages, Some(&system)).await {
                Ok(resp) => {
                    if let Err(e) = send_message(
                        &client,
                        &token,
                        msg.chat.id,
                        &resp.content,
                        Some(msg.message_id),
                        msg.message_thread_id,
                    )
                    .await
                    {
                        tracing::error!(error = %e, "failed to send reply");
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "LLM completion failed");
                    let _ = send_message(
                        &client,
                        &token,
                        msg.chat.id,
                        "⚠️ Error processing your message.",
                        Some(msg.message_id),
                        msg.message_thread_id,
                    )
                    .await;
                }
            }
        }
    }
}
