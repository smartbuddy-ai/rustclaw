//! Streaming LLM completions via Server-Sent Events (SSE).
//!
//! Provides first-class SSE streaming for Anthropic and OpenAI APIs.
//! Stream events are emitted via a tokio channel for real-time consumption.

use crate::auth::{ChatMessage, LlmAuth, Usage};
use anyhow::Result;
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::mpsc;

/// A streaming event emitted during completion.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream has started, model info available.
    Start { model: String },
    /// A text chunk arrived.
    Chunk { text: String },
    /// Stream completed with final usage stats.
    End { usage: Option<Usage> },
    /// An error occurred during streaming.
    Error { message: String },
}

// ── SSE line parser ──

fn parse_sse_lines(raw: &str) -> Vec<(Option<String>, String)> {
    let mut events = Vec::new();
    let mut event_type: Option<String> = None;
    let mut data_lines: Vec<String> = Vec::new();

    for line in raw.lines() {
        if line.starts_with("event:") {
            event_type = Some(line["event:".len()..].trim().to_string());
        } else if line.starts_with("data:") {
            data_lines.push(line["data:".len()..].trim().to_string());
        } else if line.is_empty() && !data_lines.is_empty() {
            let data = data_lines.join("\n");
            events.push((event_type.take(), data));
            data_lines.clear();
        }
    }
    // Flush remaining
    if !data_lines.is_empty() {
        events.push((event_type, data_lines.join("\n")));
    }
    events
}

// ── Anthropic SSE streaming ──

#[derive(Deserialize)]
struct AnthropicStreamStart {
    model: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicContentDelta {
    delta: Option<AnthropicDelta>,
}

#[derive(Deserialize)]
struct AnthropicDelta {
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicMessageDelta {
    usage: Option<AnthropicUsageDelta>,
}

#[derive(Deserialize)]
struct AnthropicUsageDelta {
    output_tokens: Option<u64>,
}

/// Stream an Anthropic completion, sending events to the returned receiver.
pub async fn stream_anthropic(
    auth: &LlmAuth,
    messages: &[ChatMessage],
    system: Option<&str>,
) -> Result<mpsc::Receiver<StreamEvent>> {
    let (tx, rx) = mpsc::channel(64);

    let canonical_model = crate::providers::resolve_model_alias(&auth.model).to_string();

    let mut body = serde_json::json!({
        "model": canonical_model,
        "max_tokens": 4096,
        "stream": true,
        "messages": messages.iter().map(|m| serde_json::json!({"role": &m.role, "content": &m.content})).collect::<Vec<_>>(),
    });
    if let Some(sys) = system {
        body["system"] = serde_json::Value::String(sys.to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let resp = client
        .post(format!("{}/v1/messages", auth.base_url))
        .header("x-api-key", &auth.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic streaming error {status}: {text}");
    }

    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut total_output_tokens: u64 = 0;
        let mut model_name = String::new();

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error { message: e.to_string() }).await;
                    return;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));
            let events = parse_sse_lines(&buffer);
            buffer.clear();

            for (event_type, data) in events {
                if data == "[DONE]" {
                    let _ = tx.send(StreamEvent::End {
                        usage: Some(Usage { input_tokens: 0, output_tokens: total_output_tokens }),
                    }).await;
                    return;
                }

                match event_type.as_deref() {
                    Some("message_start") => {
                        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&data) {
                            model_name = parsed.get("message")
                                .and_then(|m| m.get("model"))
                                .and_then(|m| m.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let _ = tx.send(StreamEvent::Start { model: model_name.clone() }).await;
                        }
                    }
                    Some("content_block_delta") => {
                        if let Ok(parsed) = serde_json::from_str::<AnthropicContentDelta>(&data) {
                            if let Some(delta) = parsed.delta {
                                if let Some(text) = delta.text {
                                    let _ = tx.send(StreamEvent::Chunk { text }).await;
                                }
                            }
                        }
                    }
                    Some("message_delta") => {
                        if let Ok(parsed) = serde_json::from_str::<AnthropicMessageDelta>(&data) {
                            if let Some(usage) = parsed.usage {
                                if let Some(tokens) = usage.output_tokens {
                                    total_output_tokens = tokens;
                                }
                            }
                        }
                    }
                    Some("message_stop") => {
                        let _ = tx.send(StreamEvent::End {
                            usage: Some(Usage { input_tokens: 0, output_tokens: total_output_tokens }),
                        }).await;
                        return;
                    }
                    _ => {}
                }
            }
        }

        let _ = tx.send(StreamEvent::End {
            usage: Some(Usage { input_tokens: 0, output_tokens: total_output_tokens }),
        }).await;
    });

    Ok(rx)
}

// ── OpenAI SSE streaming ──

#[derive(Deserialize)]
struct OpenAiStreamChunk {
    choices: Vec<OpenAiStreamChoice>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiStreamChoice {
    delta: Option<OpenAiStreamDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiStreamDelta {
    content: Option<String>,
}

/// Stream an OpenAI completion, sending events to the returned receiver.
pub async fn stream_openai(
    auth: &LlmAuth,
    messages: &[ChatMessage],
    system: Option<&str>,
) -> Result<mpsc::Receiver<StreamEvent>> {
    let (tx, rx) = mpsc::channel(64);

    let mut msgs = Vec::new();
    if let Some(sys) = system {
        msgs.push(serde_json::json!({"role": "system", "content": sys}));
    }
    for m in messages {
        msgs.push(serde_json::json!({"role": &m.role, "content": &m.content}));
    }

    let body = serde_json::json!({
        "model": &auth.model,
        "max_tokens": 4096,
        "stream": true,
        "messages": msgs,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;

    let resp = client
        .post(format!("{}/v1/chat/completions", auth.base_url))
        .header("Authorization", format!("Bearer {}", auth.api_key))
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI streaming error {status}: {text}");
    }

    tokio::spawn(async move {
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();
        let mut model_name = String::new();
        let mut started = false;

        while let Some(chunk_result) = stream.next().await {
            let chunk = match chunk_result {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error { message: e.to_string() }).await;
                    return;
                }
            };

            buffer.push_str(&String::from_utf8_lossy(&chunk));
            let events = parse_sse_lines(&buffer);
            buffer.clear();

            for (_event_type, data) in events {
                if data == "[DONE]" {
                    let _ = tx.send(StreamEvent::End { usage: None }).await;
                    return;
                }

                if let Ok(parsed) = serde_json::from_str::<OpenAiStreamChunk>(&data) {
                    if !started {
                        model_name = parsed.model.unwrap_or_else(|| "unknown".to_string());
                        let _ = tx.send(StreamEvent::Start { model: model_name.clone() }).await;
                        started = true;
                    }

                    for choice in &parsed.choices {
                        if let Some(delta) = &choice.delta {
                            if let Some(content) = &delta.content {
                                if !content.is_empty() {
                                    let _ = tx.send(StreamEvent::Chunk { text: content.clone() }).await;
                                }
                            }
                        }
                        if choice.finish_reason.is_some() {
                            let _ = tx.send(StreamEvent::End { usage: None }).await;
                            return;
                        }
                    }
                }
            }
        }

        let _ = tx.send(StreamEvent::End { usage: None }).await;
    });

    Ok(rx)
}

/// Collect a stream into a full completion string (useful for non-streaming callers).
pub async fn collect_stream(mut rx: mpsc::Receiver<StreamEvent>) -> Result<(String, Option<Usage>)> {
    let mut full_text = String::new();
    let mut final_usage = None;

    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::Start { .. } => {}
            StreamEvent::Chunk { text } => full_text.push_str(&text),
            StreamEvent::End { usage } => { final_usage = usage; break; }
            StreamEvent::Error { message } => anyhow::bail!("Stream error: {message}"),
        }
    }

    Ok((full_text, final_usage))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_basic() {
        let raw = "event: message_start\ndata: {\"type\":\"message_start\"}\n\nevent: content_block_delta\ndata: {\"delta\":{\"text\":\"Hello\"}}\n\n";
        let events = parse_sse_lines(raw);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0.as_deref(), Some("message_start"));
        assert_eq!(events[1].0.as_deref(), Some("content_block_delta"));
    }

    #[test]
    fn parse_sse_done() {
        let raw = "data: [DONE]\n\n";
        let events = parse_sse_lines(raw);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, "[DONE]");
    }

    #[test]
    fn parse_sse_multiline_data() {
        let raw = "data: line1\ndata: line2\n\n";
        let events = parse_sse_lines(raw);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1, "line1\nline2");
    }

    #[tokio::test]
    async fn collect_stream_aggregates_chunks() {
        let (tx, rx) = mpsc::channel(16);
        tx.send(StreamEvent::Start { model: "test".into() }).await.unwrap();
        tx.send(StreamEvent::Chunk { text: "Hello ".into() }).await.unwrap();
        tx.send(StreamEvent::Chunk { text: "world".into() }).await.unwrap();
        tx.send(StreamEvent::End { usage: Some(Usage { input_tokens: 10, output_tokens: 5 }) }).await.unwrap();
        drop(tx);

        let (text, usage) = collect_stream(rx).await.unwrap();
        assert_eq!(text, "Hello world");
        assert_eq!(usage.unwrap().output_tokens, 5);
    }

    #[tokio::test]
    async fn collect_stream_error_propagates() {
        let (tx, rx) = mpsc::channel(16);
        tx.send(StreamEvent::Start { model: "test".into() }).await.unwrap();
        tx.send(StreamEvent::Error { message: "connection lost".into() }).await.unwrap();
        drop(tx);

        let result = collect_stream(rx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("connection lost"));
    }

    #[test]
    fn stream_event_debug() {
        let evt = StreamEvent::Chunk { text: "hi".into() };
        let debug = format!("{:?}", evt);
        assert!(debug.contains("Chunk"));
    }
}
