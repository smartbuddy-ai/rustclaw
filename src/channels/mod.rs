pub mod telegram;
pub mod whatsapp;
pub mod slack;

use serde::{Deserialize, Serialize};

/// Inbound message from any channel.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel: String,
    pub chat_id: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub text: String,
    pub message_id: Option<String>,
    pub reply_to_message_id: Option<String>,
    pub thread_id: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Outbound message to any channel.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct OutboundMessage {
    pub chat_id: String,
    pub text: String,
    pub reply_to_message_id: Option<String>,
    pub thread_id: Option<String>,
}
