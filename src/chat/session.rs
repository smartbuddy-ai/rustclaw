use crate::auth::ChatMessage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Chat session with conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default = "chrono::Utc::now")]
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Session {
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            messages: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
        });
        self.updated_at = chrono::Utc::now();
    }

    /// Prune old messages to keep context manageable.
    /// Keeps the most recent `max_messages` messages.
    pub fn prune(&mut self, max_messages: usize) {
        if self.messages.len() > max_messages {
            let keep_from = self.messages.len() - max_messages;
            self.messages.drain(..keep_from);
        }
    }

    /// Get messages for LLM completion.
    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }
}

/// Session store manages conversation history persistence.
pub struct SessionStore {
    sessions_dir: PathBuf,
    max_messages: usize,
}

impl SessionStore {
    pub fn new(workspace_dir: PathBuf, max_messages: usize) -> Result<Self> {
        let sessions_dir = workspace_dir.join("sessions");
        fs::create_dir_all(&sessions_dir)?;
        Ok(Self {
            sessions_dir,
            max_messages,
        })
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.sessions_dir.join(format!("{session_id}.json"))
    }

    pub fn load(&self, session_id: &str) -> Result<Session> {
        let path = self.session_path(session_id);
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let session: Session = serde_json::from_str(&content)?;
            Ok(session)
        } else {
            Ok(Session::new(session_id.into()))
        }
    }

    pub fn save(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.session_id);
        let json = serde_json::to_string_pretty(session)?;
        fs::write(&path, json)?;
        Ok(())
    }

    pub fn add_and_save(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
    ) -> Result<Session> {
        let mut session = self.load(session_id)?;
        session.add_message(role, content);
        session.prune(self.max_messages);
        self.save(&session)?;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_new() {
        let session = Session::new("test-123".into());
        assert_eq!(session.session_id, "test-123");
        assert!(session.messages.is_empty());
    }

    #[test]
    fn test_session_add_message() {
        let mut session = Session::new("test".into());
        session.add_message("user", "Hello");
        session.add_message("assistant", "Hi there!");

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[1].content, "Hi there!");
    }

    #[test]
    fn test_session_prune() {
        let mut session = Session::new("test".into());
        for i in 0..10 {
            session.add_message("user", &format!("Message {i}"));
        }

        session.prune(5);
        assert_eq!(session.messages.len(), 5);
        assert_eq!(session.messages[0].content, "Message 5");
        assert_eq!(session.messages[4].content, "Message 9");
    }

    #[test]
    fn test_session_store() {
        let temp = TempDir::new().unwrap();
        let store = SessionStore::new(temp.path().to_path_buf(), 10).unwrap();

        let mut session = store.load("chat-123").unwrap();
        assert_eq!(session.session_id, "chat-123");
        assert!(session.messages.is_empty());

        session.add_message("user", "Test message");
        store.save(&session).unwrap();

        let loaded = store.load("chat-123").unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].content, "Test message");
    }

    #[test]
    fn test_session_store_add_and_save() {
        let temp = TempDir::new().unwrap();
        let store = SessionStore::new(temp.path().to_path_buf(), 3).unwrap();

        store.add_and_save("chat-1", "user", "Message 1").unwrap();
        store.add_and_save("chat-1", "assistant", "Reply 1").unwrap();
        store.add_and_save("chat-1", "user", "Message 2").unwrap();
        store.add_and_save("chat-1", "assistant", "Reply 2").unwrap();

        let session = store.load("chat-1").unwrap();
        // Should have pruned to last 3 messages
        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.messages[0].content, "Reply 1");
    }
}
