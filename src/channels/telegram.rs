use crate::config::{Config, TelegramConfig};
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
    callback_query: Option<CallbackQuery>,
}

#[derive(Deserialize)]
struct CallbackQuery {
    id: String,
    from: User,
    message: Option<Message>,
    data: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    message_id: i64,
    from: Option<User>,
    chat: Chat,
    text: Option<String>,
    voice: Option<Voice>,
    #[serde(default)]
    reply_to_message: Option<Box<Message>>,
    message_thread_id: Option<i64>,
}

#[derive(Deserialize)]
struct User {
    id: i64,
    first_name: String,
    last_name: Option<String>,
    username: Option<String>,
}

#[derive(Deserialize)]
struct Chat {
    id: i64,
    #[serde(rename = "type")]
    chat_type: String,
}

/// Inline keyboard button.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineKeyboardButton {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Inline keyboard markup for Telegram messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InlineKeyboardMarkup {
    pub inline_keyboard: Vec<Vec<InlineKeyboardButton>>,
}

impl InlineKeyboardMarkup {
    /// Create a simple keyboard with one button per row.
    pub fn single_column(buttons: Vec<InlineKeyboardButton>) -> Self {
        Self {
            inline_keyboard: buttons.into_iter().map(|b| vec![b]).collect(),
        }
    }

    /// Create a keyboard with all buttons in a single row.
    pub fn single_row(buttons: Vec<InlineKeyboardButton>) -> Self {
        Self {
            inline_keyboard: vec![buttons],
        }
    }
}

impl InlineKeyboardButton {
    pub fn callback(text: &str, data: &str) -> Self {
        Self { text: text.into(), callback_data: Some(data.into()), url: None }
    }
    pub fn link(text: &str, url: &str) -> Self {
        Self { text: text.into(), callback_data: None, url: Some(url.into()) }
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    reply_markup: Option<InlineKeyboardMarkup>,
}

/// Check if sender is allowed.
fn is_allowed(tg: &TelegramConfig, sender_id: i64) -> bool {
    if tg.allow_from.is_empty() {
        return true; // No allowlist = allow all
    }
    tg.allow_from.iter().any(|id| id.parse::<i64>().ok() == Some(sender_id))
}

/// Send a message via Telegram Bot API.
/// BUG-03: chat_type is required to avoid sending message_thread_id on DMs.
async fn send_message(
    client: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
    reply_to: Option<i64>,
    thread_id: Option<i64>,
    chat_type: &str,
) -> Result<()> {
    send_message_with_keyboard(client, token, chat_id, text, reply_to, thread_id, chat_type, None).await
}

/// Send a message with optional inline keyboard.
async fn send_message_with_keyboard(
    client: &reqwest::Client,
    token: &str,
    chat_id: i64,
    text: &str,
    reply_to: Option<i64>,
    thread_id: Option<i64>,
    chat_type: &str,
    keyboard: Option<InlineKeyboardMarkup>,
) -> Result<()> {
    let effective_thread_id = if chat_type == "private" { None } else { thread_id };

    let chunks = chunk_text(text, 4096);
    for (i, chunk) in chunks.iter().enumerate() {
        // Only attach keyboard to the last chunk
        let markup = if i == chunks.len() - 1 { keyboard.clone() } else { None };

        let body = SendMessageBody {
            chat_id,
            text: chunk.clone(),
            parse_mode: Some("Markdown".into()),
            reply_to_message_id: reply_to,
            message_thread_id: effective_thread_id,
            reply_markup: markup.clone(),
        };

        let resp = client
            .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let body_plain = SendMessageBody {
                chat_id,
                text: chunk.clone(),
                parse_mode: None,
                reply_to_message_id: reply_to,
                message_thread_id: effective_thread_id,
                reply_markup: markup,
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

/// Answer a callback query (acknowledge the button press).
async fn answer_callback_query(
    client: &reqwest::Client,
    token: &str,
    callback_query_id: &str,
    text: Option<&str>,
) -> Result<()> {
    let mut body = serde_json::json!({ "callback_query_id": callback_query_id });
    if let Some(t) = text {
        body["text"] = serde_json::Value::String(t.to_string());
    }
    client
        .post(format!("https://api.telegram.org/bot{token}/answerCallbackQuery"))
        .json(&body)
        .send()
        .await?;
    Ok(())
}

/// Handle a callback query from an inline button press.
async fn handle_callback_query(
    cfg: &Config,
    client: &reqwest::Client,
    token: &str,
    cq: &CallbackQuery,
) -> Result<()> {
    let data = cq.data.as_deref().unwrap_or("");
    let chat_id = cq.message.as_ref().map(|m| m.chat.id).unwrap_or(0);
    let chat_type = cq.message.as_ref().map(|m| m.chat.chat_type.as_str()).unwrap_or("private");

    // Acknowledge the callback
    answer_callback_query(client, token, &cq.id, Some("Processing...")).await?;

    // Process callback data as a message
    let session_id = format!("telegram:{chat_id}");
    let enriched = format!("from={} callback_data={data}", cq.from.first_name);
    let reply = match crate::chat::send_with_session(cfg, &session_id, &enriched).await {
        Ok(r) => r,
        Err(_) => "⚠️ Error processing callback.".into(),
    };

    send_message(client, token, chat_id, &reply, None, None, chat_type).await?;
    Ok(())
}

// ── Voice note types ──

#[derive(Deserialize)]
struct Voice {
    file_id: String,
    duration: Option<i64>,
}

#[derive(Deserialize)]
struct TgFile {
    file_path: Option<String>,
}

pub(crate) fn chunk_text(text: &str, limit: usize) -> Vec<String> {
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
        // Try to split at a newline first, then whitespace
        let split_window = &remaining[..limit];
        let split_at = split_window
            .rfind('\n')
            .or_else(|| split_window.rfind(' '))
            .unwrap_or(limit);
        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start();
    }
    chunks
}

async fn process_message(cfg: &Config, msg: &Message) -> String {
    let text = msg.text.clone().unwrap_or_default();
    if text.trim() == "/start" {
        return "👋 Rustclaw is online. Send me a message and I’ll reply.".to_string();
    }

    let reply_context = msg
        .reply_to_message
        .as_ref()
        .and_then(|m| m.text.clone())
        .map(|t| format!("\n[reply_context]\n{}\n[/reply_context]\n", t))
        .unwrap_or_default();

    let sender = msg.from.as_ref().map(|u| {
        if let Some(username) = &u.username {
            format!("{} (@{})", u.first_name, username)
        } else {
            u.first_name.clone()
        }
    }).unwrap_or_else(|| "unknown".to_string());

    let enriched = format!("from={sender} chat_type={}\n{}{text}", msg.chat.chat_type, reply_context);
    let session_id = format!("telegram:{}", msg.chat.id);
    match crate::chat::send_with_session(cfg, &session_id, &enriched).await {
        Ok(reply) => reply,
        Err(_) => "⚠️ Error processing your message.".into(),
    }
}

/// Run Telegram long-polling loop.
pub async fn run(cfg: &Config, tg: &TelegramConfig) -> Result<()> {
    let token = resolve_token(tg)?;
    let client = reqwest::Client::new();
    let mut offset: Option<i64> = None;

    tracing::info!(channel = "telegram", "starting polling");

    loop {
        let mut url = format!("https://api.telegram.org/bot{token}/getUpdates?timeout=30");
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

            // Handle callback queries (inline button presses)
            if let Some(cq) = update.callback_query {
                let sender_id = cq.from.id;
                if !is_allowed(tg, sender_id) {
                    continue;
                }
                if let Err(e) = handle_callback_query(cfg, &client, &token, &cq).await {
                    tracing::error!(error = %e, "failed to handle callback query");
                }
                continue;
            }

            let msg = match update.message {
                Some(m) => m,
                None => continue,
            };

            let sender_id = msg.from.as_ref().map(|u| u.id).unwrap_or(0);
            if !is_allowed(tg, sender_id) {
                tracing::debug!(sender_id, "ignoring message from non-allowed sender");
                continue;
            }

            // Handle voice messages
            if let Some(voice) = &msg.voice {
                tracing::info!(chat_id = msg.chat.id, file_id = %voice.file_id, "received voice message");
                let transcript = transcribe_voice(&client, &token, &voice.file_id).await;
                let text = match transcript {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(error = %e, "voice transcription failed");
                        "⚠️ Could not transcribe voice message.".to_string()
                    }
                };
                // Process transcription as a normal message
                let session_id = format!("telegram:{}", msg.chat.id);
                let enriched = format!("[voice_transcript] {text}");
                let reply = match crate::chat::send_with_session(cfg, &session_id, &enriched).await {
                    Ok(r) => r,
                    Err(_) => "⚠️ Error processing voice message.".into(),
                };
                let _ = send_message(&client, &token, msg.chat.id, &reply, Some(msg.message_id), msg.message_thread_id, &msg.chat.chat_type).await;
                continue;
            }

            let text = match msg.text {
                Some(ref t) => t.clone(),
                None => continue,
            };

            tracing::info!(chat_id = msg.chat.id, text = %text, "received telegram message");

            let reply = process_message(cfg, &msg).await;
            if let Err(e) = send_message(
                &client,
                &token,
                msg.chat.id,
                &reply,
                Some(msg.message_id),
                msg.message_thread_id,
                &msg.chat.chat_type,
            )
            .await
            {
                tracing::error!(error = %e, "failed to send reply");
            }
        }
    }
}

/// Download and transcribe a voice message via Whisper.
async fn transcribe_voice(
    client: &reqwest::Client,
    token: &str,
    file_id: &str,
) -> Result<String> {
    // 1) Get file path from Telegram
    let file_resp: TgResponse<TgFile> = client
        .get(format!("https://api.telegram.org/bot{token}/getFile?file_id={file_id}"))
        .send()
        .await?
        .json()
        .await?;

    let file_path = file_resp
        .result
        .and_then(|f| f.file_path)
        .ok_or_else(|| anyhow::anyhow!("no file_path in getFile response"))?;

    // 2) Download the file
    let download_url = format!("https://api.telegram.org/file/bot{token}/{file_path}");
    let audio_bytes = client.get(&download_url).send().await?.bytes().await?;

    // 3) Save to temp file
    let tmp_path = std::env::temp_dir().join(format!("rustclaw_voice_{file_id}.ogg"));
    std::fs::write(&tmp_path, &audio_bytes)?;

    // 4) Run Whisper for transcription
    let output = tokio::process::Command::new("whisper")
        .args(["--model", "turbo", "--output_format", "txt", "--output_dir"])
        .arg(std::env::temp_dir())
        .arg(&tmp_path)
        .output()
        .await?;

    // 5) Read the transcript
    let txt_path = tmp_path.with_extension("txt");
    let transcript = if txt_path.exists() {
        std::fs::read_to_string(&txt_path).unwrap_or_default().trim().to_string()
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Whisper transcription failed: {stderr}");
    };

    // Cleanup
    let _ = std::fs::remove_file(&tmp_path);
    let _ = std::fs::remove_file(&txt_path);

    Ok(transcript)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunking_respects_limit() {
        let text = "a".repeat(9000);
        let chunks = chunk_text(&text, 4096);
        assert!(chunks.len() >= 3);
        assert!(chunks.iter().all(|c| c.len() <= 4096));
    }

    #[test]
    fn allowlist_works() {
        let cfg = TelegramConfig {
            enabled: true,
            bot_token: None,
            allow_from: vec!["123".into()],
            webhook_url: None,
        };
        assert!(is_allowed(&cfg, 123));
        assert!(!is_allowed(&cfg, 456));
    }

    #[test]
    fn inline_keyboard_single_column() {
        let kb = InlineKeyboardMarkup::single_column(vec![
            InlineKeyboardButton::callback("Button 1", "cb_1"),
            InlineKeyboardButton::callback("Button 2", "cb_2"),
        ]);
        assert_eq!(kb.inline_keyboard.len(), 2);
        assert_eq!(kb.inline_keyboard[0][0].text, "Button 1");
        assert_eq!(kb.inline_keyboard[0][0].callback_data, Some("cb_1".into()));
    }

    #[test]
    fn inline_keyboard_single_row() {
        let kb = InlineKeyboardMarkup::single_row(vec![
            InlineKeyboardButton::callback("A", "a"),
            InlineKeyboardButton::callback("B", "b"),
        ]);
        assert_eq!(kb.inline_keyboard.len(), 1);
        assert_eq!(kb.inline_keyboard[0].len(), 2);
    }

    #[test]
    fn inline_keyboard_link_button() {
        let btn = InlineKeyboardButton::link("Visit", "https://example.com");
        assert!(btn.url.is_some());
        assert!(btn.callback_data.is_none());
    }

    #[test]
    fn inline_keyboard_serializes() {
        let kb = InlineKeyboardMarkup::single_row(vec![
            InlineKeyboardButton::callback("Click", "data"),
        ]);
        let json = serde_json::to_string(&kb).unwrap();
        assert!(json.contains("inline_keyboard"));
        assert!(json.contains("callback_data"));
    }

    #[test]
    fn send_message_body_with_keyboard_serializes() {
        let body = SendMessageBody {
            chat_id: 123,
            text: "Hello".into(),
            parse_mode: None,
            reply_to_message_id: None,
            message_thread_id: None,
            reply_markup: Some(InlineKeyboardMarkup::single_row(vec![
                InlineKeyboardButton::callback("OK", "ok"),
            ])),
        };
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("reply_markup"));
    }

    // BUG-03: message_thread_id should be None for private (DM) chats
    #[test]
    fn send_message_body_omits_thread_id_for_private_chat() {
        // Simulate what send_message does internally for a private chat
        let chat_type = "private";
        let thread_id = Some(42_i64);
        let effective = if chat_type == "private" { None } else { thread_id };
        assert!(effective.is_none(), "thread_id should be None for private chats");

        // For group chats, thread_id should be preserved
        let chat_type = "supergroup";
        let effective = if chat_type == "private" { None } else { thread_id };
        assert_eq!(effective, Some(42), "thread_id should be kept for group chats");
    }
}
