use crate::auth::{self, ChatMessage};
use crate::config::Config;
use crate::workspace;
use anyhow::Result;

/// Send a single chat message and get a response.
pub async fn send(cfg: &Config, message: &str, _channel: Option<&str>) -> Result<String> {
    let system = workspace::build_system_prompt(cfg)?;
    let messages = vec![ChatMessage {
        role: "user".into(),
        content: message.into(),
    }];

    let resp = auth::complete(cfg, &messages, Some(&system)).await?;

    if let Some(usage) = &resp.usage {
        tracing::debug!(
            model = %resp.model,
            input = usage.input_tokens,
            output = usage.output_tokens,
            "completion"
        );
    }

    Ok(resp.content)
}
