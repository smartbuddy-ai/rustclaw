use crate::config::{Config, DiscordConfig};
use anyhow::Result;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{Duration, Instant};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

const GATEWAY_URL: &str = "wss://gateway.discord.gg/?v=10&encoding=json";
const INTENT_GUILD_MESSAGES: u64 = 1 << 9;

#[derive(Debug, Clone, Deserialize)]
struct DiscordMessageCreate {
    id: String,
    channel_id: String,
    content: String,
    author: DiscordAuthor,
}

#[derive(Debug, Clone, Deserialize)]
struct DiscordAuthor {
    id: String,
    bot: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GatewayPayload {
    op: u64,
    #[serde(default)]
    d: serde_json::Value,
    #[serde(default)]
    s: Option<u64>,
    #[serde(default)]
    t: Option<String>,
}

#[derive(Debug, Serialize)]
struct IdentifyPayload<'a> {
    token: &'a str,
    intents: u64,
    properties: IdentifyProperties<'a>,
}

#[derive(Debug, Serialize)]
struct IdentifyProperties<'a> {
    os: &'a str,
    browser: &'a str,
    device: &'a str,
}

#[derive(Debug, Serialize)]
struct ResumePayload<'a> {
    token: &'a str,
    session_id: &'a str,
    seq: u64,
}

async fn send_message(client: &reqwest::Client, token: &str, channel_id: &str, content: &str) -> Result<()> {
    let payload = serde_json::json!({ "content": content });
    let resp = client
        .post(format!("https://discord.com/api/v10/channels/{channel_id}/messages"))
        .header("Authorization", format!("Bot {token}"))
        .json(&payload)
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("discord send message failed: {}", resp.status());
    }
    Ok(())
}

fn parse_message_create(v: &serde_json::Value) -> Option<DiscordMessageCreate> {
    serde_json::from_value(v.clone()).ok()
}

pub async fn run(cfg: &Config, dc: &DiscordConfig) -> Result<()> {
    let token = dc
        .bot_token
        .clone()
        .or_else(|| std::env::var("DISCORD_BOT_TOKEN").ok())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("DISCORD_BOT_TOKEN not set"))?;

    let http = reqwest::Client::new();
    let mut session_id: Option<String> = None;
    let mut seq: Option<u64> = None;

    loop {
        let connected = run_gateway_loop(cfg, dc, &token, &http, &mut session_id, &mut seq).await;
        if let Err(e) = connected {
            tracing::warn!(error = %e, "discord gateway loop ended, reconnecting");
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn run_gateway_loop(
    cfg: &Config,
    dc: &DiscordConfig,
    token: &str,
    http: &reqwest::Client,
    session_id: &mut Option<String>,
    seq: &mut Option<u64>,
) -> Result<()> {
    let (ws_stream, _) = connect_async(GATEWAY_URL).await?;
    let (mut write, mut read) = ws_stream.split();

    let hello_msg = read.next().await.ok_or_else(|| anyhow::anyhow!("discord gateway closed before hello"))??;
    let hello_text = hello_msg.into_text()?;
    let hello: GatewayPayload = serde_json::from_str(&hello_text)?;
    let heartbeat_interval_ms = hello
        .d
        .get("heartbeat_interval")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| anyhow::anyhow!("heartbeat_interval missing"))?;

    let identify_or_resume = if let (Some(sid), Some(last_seq)) = (session_id.as_ref(), seq.as_ref()) {
        serde_json::json!({"op": 6, "d": ResumePayload{ token, session_id: sid, seq: *last_seq }})
    } else {
        serde_json::json!({"op": 2, "d": IdentifyPayload {
            token,
            intents: INTENT_GUILD_MESSAGES,
            properties: IdentifyProperties { os: "linux", browser: "rustclaw", device: "rustclaw" }
        }})
    };
    write.send(Message::Text(identify_or_resume.to_string())).await?;

    let mut next_heartbeat = Instant::now() + Duration::from_millis(heartbeat_interval_ms);

    loop {
        tokio::select! {
            _ = tokio::time::sleep_until(next_heartbeat.into()) => {
                let hb = serde_json::json!({"op": 1, "d": *seq});
                write.send(Message::Text(hb.to_string())).await?;
                next_heartbeat = Instant::now() + Duration::from_millis(heartbeat_interval_ms);
            }
            maybe = read.next() => {
                let msg = match maybe {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => return Err(e.into()),
                    None => return Err(anyhow::anyhow!("discord websocket closed")),
                };

                if let Message::Close(_) = msg {
                    return Err(anyhow::anyhow!("discord websocket closed"));
                }
                let text = match msg.into_text() {
                    Ok(t) => t,
                    Err(_) => continue,
                };
                let payload: GatewayPayload = serde_json::from_str(&text)?;
                if let Some(s) = payload.s {
                    *seq = Some(s);
                }

                match payload.op {
                    0 => {
                        if payload.t.as_deref() == Some("READY") {
                            if let Some(sid) = payload.d.get("session_id").and_then(|v| v.as_str()) {
                                *session_id = Some(sid.to_string());
                            }
                        }
                        if payload.t.as_deref() == Some("MESSAGE_CREATE") {
                            if let Some(msg) = parse_message_create(&payload.d) {
                                if msg.author.bot.unwrap_or(false) { continue; }
                                if !dc.channel_ids.is_empty() && !dc.channel_ids.iter().any(|c| c == &msg.channel_id) {
                                    continue;
                                }
                                let sid = format!("discord:{}:{}", msg.channel_id, msg.author.id);
                                let reply = crate::chat::send_with_session(cfg, &sid, &msg.content).await.unwrap_or_else(|e| {
                                    tracing::error!(error = %e, "discord LLM error");
                                    "⚠️ Error processing your message.".into()
                                });
                                if let Err(e) = send_message(http, token, &msg.channel_id, &reply).await {
                                    tracing::error!(error = %e, "discord send failed");
                                }
                            }
                        }
                    }
                    1 => {
                        let hb = serde_json::json!({"op": 1, "d": *seq});
                        write.send(Message::Text(hb.to_string())).await?;
                    }
                    11 => {
                        // HEARTBEAT_ACK
                    }
                    _ => {}
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_message_create_dispatch() {
        let d = serde_json::json!({
            "id": "m1",
            "channel_id": "c1",
            "content": "hello",
            "author": { "id": "u1", "bot": false }
        });
        let msg = parse_message_create(&d).unwrap();
        assert_eq!(msg.id, "m1");
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.author.id, "u1");
    }

    #[test]
    fn ignores_invalid_payload() {
        let d = serde_json::json!({"foo":"bar"});
        assert!(parse_message_create(&d).is_none());
    }
}
