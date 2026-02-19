pub mod fts;
pub mod vector;

// Ported from zeroclaw src/memory/sqlite.rs (simplified for rustclaw)
use crate::config::Config;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub key: String,
    pub content: String,
    pub category: String,
    pub created_at: String,
    pub updated_at: String,
}

pub struct SqliteMemory {
    db_path: PathBuf,
    fts_enabled: bool,
}

impl SqliteMemory {
    pub fn from_config(cfg: &Config) -> Result<Self> {
        let db_path = cfg.workspace_dir.join(&cfg.memory.sqlite_path);
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut this = Self { db_path, fts_enabled: false };
        this.init_schema()?;
        Ok(this)
    }

    fn connect(&self) -> Result<Connection> {
        Ok(Connection::open(&self.db_path)?)
    }

    fn init_schema(&mut self) -> Result<()> {
        let conn = self.connect()?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                key TEXT UNIQUE NOT NULL,
                content TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'core',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category);
             CREATE INDEX IF NOT EXISTS idx_memories_updated_at ON memories(updated_at DESC);",
        )?;
        // Feature 9: Initialize FTS5 if available
        self.fts_enabled = fts::init_fts5(&conn).unwrap_or(false);
        if self.fts_enabled {
            // Rebuild index from existing data
            let _ = fts::rebuild_fts_index(&conn);
        }
        Ok(())
    }

    pub fn upsert(&self, key: &str, content: &str, category: &str) -> Result<()> {
        let conn = self.connect()?;
        let now = Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO memories (id, key, content, category, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(key) DO UPDATE SET
               content = excluded.content,
               category = excluded.category,
               updated_at = excluded.updated_at",
            params![id, key, content, category, now, now],
        )?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<MemoryRecord>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, key, content, category, created_at, updated_at
             FROM memories WHERE key = ?1",
        )?;

        let row = stmt.query_row(params![key], |r| {
            Ok(MemoryRecord {
                id: r.get(0)?,
                key: r.get(1)?,
                content: r.get(2)?,
                category: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        });

        match row {
            Ok(record) => Ok(Some(record)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.connect()?;

        // Feature 9: Use FTS5 if available, fallback to LIKE
        if self.fts_enabled && !query.is_empty() {
            match fts::fts5_search(&conn, query, limit, 3) {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => { /* fall through to LIKE */ }
                Err(e) => {
                    tracing::debug!("FTS5 search failed, falling back to LIKE: {e}");
                }
            }
        }

        let sql = "SELECT id, key, content, category, created_at, updated_at
                   FROM memories
                   WHERE key LIKE ?1 OR content LIKE ?1
                   ORDER BY updated_at DESC
                   LIMIT ?2";
        let mut stmt = conn.prepare(sql)?;
        let like = format!("%{query}%");

        let rows = stmt.query_map(params![like, limit as i64], |r| {
            Ok(MemoryRecord {
                id: r.get(0)?,
                key: r.get(1)?,
                content: r.get(2)?,
                category: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn forget(&self, key: &str) -> Result<bool> {
        let conn = self.connect()?;
        let affected = conn.execute("DELETE FROM memories WHERE key = ?1", params![key])?;
        Ok(affected > 0)
    }

    pub fn db_path(&self) -> PathBuf {
        self.db_path.clone()
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthConfig, ChannelsConfig, Config, CronConfig, MemoryConfig, NodeConfig, TunnelConfig};
    use tempfile::TempDir;

    fn test_cfg(tmp: &TempDir) -> Config {
        Config {
            workspace_dir: tmp.path().to_path_buf(),
            auth: AuthConfig::default(),
            channels: ChannelsConfig::default(),
            cron: CronConfig::default(),
            node: NodeConfig::default(),
            memory: MemoryConfig::default(),
            tunnel: TunnelConfig::default(),
            gateway: crate::config::GatewayConfig::default(),
            tools: crate::config::ToolsConfig::default(),
        }
    }

    #[test]
    fn sqlite_memory_upsert_get_search() {
        let tmp = TempDir::new().unwrap();
        let cfg = test_cfg(&tmp);
        let mem = SqliteMemory::from_config(&cfg).unwrap();

        mem.upsert("project", "rustclaw parity work", "core").unwrap();
        mem.upsert("todo", "implement reliable provider", "daily").unwrap();

        let p = mem.get("project").unwrap().unwrap();
        assert_eq!(p.category, "core");

        let hits = mem.search("provider", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].key, "todo");

        assert!(mem.forget("todo").unwrap());
        assert!(mem.get("todo").unwrap().is_none());
    }
}
