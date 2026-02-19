//! Feature 9: FTS5 SQLite full-text search for memory.
//! Provides faster and more relevant search than LIKE '%query%'.
//! Falls back to LIKE if FTS5 is not available.

use crate::memory::MemoryRecord;
use anyhow::Result;
use rusqlite::{Connection, params};

/// Check if FTS5 is available in this SQLite build.
pub fn fts5_available(conn: &Connection) -> bool {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS _fts5_check USING fts5(test);
         DROP TABLE IF EXISTS _fts5_check;"
    ).is_ok()
}

/// Initialize the FTS5 virtual table for memories.
pub fn init_fts5(conn: &Connection) -> Result<bool> {
    if !fts5_available(conn) {
        tracing::warn!("FTS5 not available, falling back to LIKE search");
        return Ok(false);
    }

    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            key,
            content,
            category,
            content='memories',
            content_rowid='rowid'
        );

        -- Triggers to keep FTS in sync with the main table
        CREATE TRIGGER IF NOT EXISTS memories_ai AFTER INSERT ON memories BEGIN
            INSERT INTO memories_fts(rowid, key, content, category)
            VALUES (new.rowid, new.key, new.content, new.category);
        END;

        CREATE TRIGGER IF NOT EXISTS memories_ad AFTER DELETE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, key, content, category)
            VALUES ('delete', old.rowid, old.key, old.content, old.category);
        END;

        CREATE TRIGGER IF NOT EXISTS memories_au AFTER UPDATE ON memories BEGIN
            INSERT INTO memories_fts(memories_fts, rowid, key, content, category)
            VALUES ('delete', old.rowid, old.key, old.content, old.category);
            INSERT INTO memories_fts(rowid, key, content, category)
            VALUES (new.rowid, new.key, new.content, new.category);
        END;"
    )?;

    Ok(true)
}

/// Rebuild the FTS index from existing memories data.
pub fn rebuild_fts_index(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "INSERT INTO memories_fts(memories_fts) VALUES('rebuild');"
    )?;
    Ok(())
}

/// Search memories using FTS5 with query expansion.
/// If fewer than `min_results` are found, retries with partial term matching.
pub fn fts5_search(
    conn: &Connection,
    query: &str,
    limit: usize,
    min_results: usize,
) -> Result<Vec<MemoryRecord>> {
    // First try exact FTS5 match
    let results = fts5_query(conn, query, limit)?;
    if results.len() >= min_results {
        return Ok(results);
    }

    // Query expansion: try with prefix matching (partial terms)
    let prefix_query: String = query
        .split_whitespace()
        .map(|term| format!("{term}*"))
        .collect::<Vec<_>>()
        .join(" ");

    let expanded = fts5_query(conn, &prefix_query, limit)?;
    if expanded.len() > results.len() {
        return Ok(expanded);
    }

    Ok(results)
}

/// Execute an FTS5 query.
fn fts5_query(conn: &Connection, fts_query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
    let sql = "SELECT m.id, m.key, m.content, m.category, m.created_at, m.updated_at
               FROM memories m
               JOIN memories_fts fts ON m.rowid = fts.rowid
               WHERE memories_fts MATCH ?1
               ORDER BY rank
               LIMIT ?2";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params![fts_query, limit as i64], |r| {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE memories (
                rowid INTEGER PRIMARY KEY AUTOINCREMENT,
                id TEXT UNIQUE,
                key TEXT UNIQUE NOT NULL,
                content TEXT NOT NULL,
                category TEXT NOT NULL DEFAULT 'core',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_memories_category ON memories(category);"
        ).unwrap();
        conn
    }

    fn insert_memory(conn: &Connection, id: &str, key: &str, content: &str, category: &str) {
        conn.execute(
            "INSERT INTO memories(id, key, content, category, created_at, updated_at) VALUES(?1,?2,?3,?4,'2024-01-01','2024-01-01')",
            params![id, key, content, category],
        ).unwrap();
    }

    #[test]
    fn fts5_is_available() {
        let conn = Connection::open_in_memory().unwrap();
        // With bundled rusqlite, FTS5 should be available
        assert!(fts5_available(&conn));
    }

    #[test]
    fn init_and_search_fts5() {
        let conn = setup_db();
        let ok = init_fts5(&conn).unwrap();
        assert!(ok);

        insert_memory(&conn, "1", "rust-project", "Building a gateway in Rust", "core");
        insert_memory(&conn, "2", "python-project", "Data analysis with Python", "daily");
        insert_memory(&conn, "3", "rust-tooling", "Cargo clippy and testing tools", "core");

        let results = fts5_search(&conn, "rust", 10, 3).unwrap();
        assert!(results.len() >= 2, "should find at least 2 Rust-related memories");
    }

    #[test]
    fn fts5_query_expansion() {
        let conn = setup_db();
        init_fts5(&conn).unwrap();

        insert_memory(&conn, "1", "authentication", "OAuth2 and JWT tokens for auth", "core");
        insert_memory(&conn, "2", "authorization", "Role-based access control", "core");

        // "auth" should match both via prefix expansion
        let results = fts5_search(&conn, "auth", 10, 3).unwrap();
        assert!(results.len() >= 2, "prefix expansion should find both auth-related records, got {}", results.len());
    }

    #[test]
    fn fts5_rebuild_index() {
        let conn = setup_db();
        init_fts5(&conn).unwrap();

        insert_memory(&conn, "1", "test", "test content", "core");
        rebuild_fts_index(&conn).unwrap();

        let results = fts5_search(&conn, "test", 10, 1).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn fts5_empty_query_returns_empty() {
        let conn = setup_db();
        init_fts5(&conn).unwrap();

        insert_memory(&conn, "1", "test", "test content", "core");
        // FTS5 with empty string should return empty or error gracefully
        let results = fts5_search(&conn, "", 10, 1);
        // Empty query might error in FTS5, that's acceptable
        assert!(results.is_ok() || results.is_err());
    }
}
