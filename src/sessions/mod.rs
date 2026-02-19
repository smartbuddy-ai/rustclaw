use anyhow::Result;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub agent_id: String,
    pub channel: String,
    pub peer: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

pub struct SessionStore { conn: Connection }

impl SessionStore {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        if let Some(p) = path.parent() { std::fs::create_dir_all(p)?; }
        let conn = Connection::open(path)?;
        conn.execute_batch("CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY, agent_id TEXT, channel TEXT, peer TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
                           CREATE TABLE IF NOT EXISTS session_messages(id TEXT PRIMARY KEY, session_id TEXT, role TEXT, content TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP);")?;
        Ok(Self { conn })
    }
    pub fn create_or_get(&self, agent_id: &str, channel: &str, peer: &str) -> Result<Session> {
        let mut st = self.conn.prepare("SELECT id,agent_id,channel,peer,created_at FROM sessions WHERE agent_id=?1 AND channel=?2 AND peer=?3 LIMIT 1")?;
        let existing = st.query_row(params![agent_id, channel, peer], |r| Ok(Session { id: r.get(0)?, agent_id: r.get(1)?, channel: r.get(2)?, peer: r.get(3)?, created_at: r.get(4)? }));
        if let Ok(s) = existing { return Ok(s); }
        let id = uuid::Uuid::new_v4().to_string();
        self.conn.execute("INSERT INTO sessions(id,agent_id,channel,peer) VALUES(?1,?2,?3,?4)", params![id, agent_id, channel, peer])?;
        self.get(&id)?.ok_or_else(|| anyhow::anyhow!("session missing"))
    }
    pub fn get(&self, id: &str) -> Result<Option<Session>> {
        let mut st = self.conn.prepare("SELECT id,agent_id,channel,peer,created_at FROM sessions WHERE id=?1")?;
        let row = st.query_row(params![id], |r| Ok(Session { id: r.get(0)?, agent_id: r.get(1)?, channel: r.get(2)?, peer: r.get(3)?, created_at: r.get(4)? }));
        match row { Ok(s) => Ok(Some(s)), Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None), Err(e)=>Err(e.into()) }
    }
    pub fn list(&self) -> Result<Vec<Session>> {
        let mut st = self.conn.prepare("SELECT id,agent_id,channel,peer,created_at FROM sessions ORDER BY created_at DESC")?;
        let rows = st.query_map([], |r| Ok(Session { id: r.get(0)?, agent_id: r.get(1)?, channel: r.get(2)?, peer: r.get(3)?, created_at: r.get(4)? }))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
    pub fn append_message(&self, session_id: &str, role: &str, content: &str) -> Result<()> {
        self.conn.execute("INSERT INTO session_messages(id,session_id,role,content) VALUES(?1,?2,?3,?4)", params![uuid::Uuid::new_v4().to_string(), session_id, role, content])?;
        Ok(())
    }
    pub fn history(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>> {
        let mut st = self.conn.prepare("SELECT id,session_id,role,content,created_at FROM session_messages WHERE session_id=?1 ORDER BY created_at DESC LIMIT ?2")?;
        let rows = st.query_map(params![session_id, limit as i64], |r| Ok(SessionMessage { id:r.get(0)?, session_id:r.get(1)?, role:r.get(2)?, content:r.get(3)?, created_at:r.get(4)? }))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
    pub fn compact(&self, session_id: &str, keep_last: usize) -> Result<()> {
        let msgs = self.history(session_id, 10_000)?;
        if msgs.len() <= keep_last { return Ok(()); }
        let old = &msgs[keep_last..];
        let summary = format!("Summary of {} old messages", old.len());
        self.conn.execute("DELETE FROM session_messages WHERE session_id=?1 AND id IN (SELECT id FROM session_messages WHERE session_id=?1 ORDER BY created_at DESC LIMIT -1 OFFSET ?2)", params![session_id, keep_last as i64])?;
        self.append_message(session_id, "system", &summary)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn create_append_history_compact() {
        let tmp = tempfile::tempdir().unwrap();
        let store = SessionStore::open(&tmp.path().join("s.db")).unwrap();
        let s = store.create_or_get("a1", "telegram", "u1").unwrap();
        store.append_message(&s.id, "user", "hi").unwrap();
        store.append_message(&s.id, "assistant", "hello").unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(store.history(&s.id, 10).unwrap().len(), 2);
        store.compact(&s.id, 1).unwrap();
        assert!(!store.history(&s.id, 10).unwrap().is_empty());
    }
}
