pub mod session;

use crate::auth::{self, ChatMessage};
use crate::config::Config;
use crate::workspace;
use anyhow::Result;
use session::SessionStore;

/// Send a single chat message and get a response (stateless, single-turn).
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

/// Send a chat message with conversation history (stateful, multi-turn).
pub async fn send_with_session(
    cfg: &Config,
    session_id: &str,
    message: &str,
) -> Result<String> {
    let store = SessionStore::new(cfg.workspace_dir.clone(), 20)?;
    
    // Add user message to session
    let session = store.add_and_save(session_id, "user", message)?;

    // Build system prompt
    let system = workspace::build_system_prompt(cfg)?;

    // Send conversation history to LLM
    let resp = auth::complete(cfg, session.get_messages(), Some(&system)).await?;

    if let Some(usage) = &resp.usage {
        tracing::debug!(
            model = %resp.model,
            input = usage.input_tokens,
            output = usage.output_tokens,
            "completion"
        );
    }

    // Save assistant response
    store.add_and_save(session_id, "assistant", &resp.content)?;

    Ok(resp.content)
}
