use super::{ChatMessage, CompletionResponse, LlmAuth, Usage};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    model: String,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    _type: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: u64,
    output_tokens: u64,
}

pub async fn complete(
    auth: &LlmAuth,
    messages: &[ChatMessage],
    system: Option<&str>,
) -> anyhow::Result<CompletionResponse> {
    let client = reqwest::Client::new();

    let body = AnthropicRequest {
        model: auth.model.clone(),
        max_tokens: 4096,
        system: system.map(String::from),
        messages: messages
            .iter()
            .map(|m| AnthropicMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect(),
    };

    let resp = client
        .post(format!("{}/v1/messages", auth.base_url))
        .header("x-api-key", &auth.api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Anthropic API error {status}: {text}");
    }

    let data: AnthropicResponse = resp.json().await?;
    let content = data
        .content
        .iter()
        .filter_map(|b| b.text.as_deref())
        .collect::<Vec<_>>()
        .join("");

    Ok(CompletionResponse {
        content,
        model: data.model,
        usage: data.usage.map(|u| Usage {
            input_tokens: u.input_tokens,
            output_tokens: u.output_tokens,
        }),
    })
}
