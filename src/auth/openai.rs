use super::{ChatMessage, CompletionResponse, LlmAuth, Usage};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    max_tokens: u32,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<Choice>,
    model: String,
    usage: Option<OpenAiUsage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

pub async fn complete(
    auth: &LlmAuth,
    messages: &[ChatMessage],
    system: Option<&str>,
) -> anyhow::Result<CompletionResponse> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()?;

    let mut msgs: Vec<OpenAiMessage> = Vec::new();
    if let Some(sys) = system {
        msgs.push(OpenAiMessage {
            role: "system".into(),
            content: sys.into(),
        });
    }
    for m in messages {
        msgs.push(OpenAiMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        });
    }

    let body = OpenAiRequest {
        model: auth.model.clone(),
        messages: msgs,
        max_tokens: 4096,
    };

    let resp = client
        .post(format!("{}/v1/chat/completions", auth.base_url))
        .header("Authorization", format!("Bearer {}", auth.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("OpenAI API error {status}: {text}");
    }

    let data: OpenAiResponse = resp.json().await?;
    let content = data
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default();

    Ok(CompletionResponse {
        content,
        model: data.model,
        usage: data.usage.map(|u| Usage {
            input_tokens: u.prompt_tokens,
            output_tokens: u.completion_tokens,
        }),
    })
}
