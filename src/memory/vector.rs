use anyhow::Result;
use rusqlite::{Connection, params};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct VectorHit {
    pub doc_id: String,
    pub chunk: String,
    pub score: f32,
}

pub struct VectorStore {
    conn: Connection,
}

impl VectorStore {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS memory_vectors (
                id TEXT PRIMARY KEY,
                doc_id TEXT NOT NULL,
                chunk TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_memory_vectors_doc ON memory_vectors(doc_id);",
        )?;
        Ok(Self { conn })
    }

    pub fn insert_embedding(&self, doc_id: &str, chunk: &str, embedding: &[f32]) -> Result<()> {
        let id = uuid::Uuid::new_v4().to_string();
        let blob = to_blob(embedding);
        self.conn.execute(
            "INSERT INTO memory_vectors(id, doc_id, chunk, embedding, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![id, doc_id, chunk, blob],
        )?;
        Ok(())
    }

    pub fn semantic_search(&self, query: &[f32], limit: usize) -> Result<Vec<VectorHit>> {
        let mut stmt = self.conn.prepare("SELECT doc_id, chunk, embedding FROM memory_vectors")?;
        let rows = stmt.query_map([], |r| {
            let doc_id: String = r.get(0)?;
            let chunk: String = r.get(1)?;
            let emb_blob: Vec<u8> = r.get(2)?;
            Ok((doc_id, chunk, from_blob(&emb_blob)))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (doc_id, chunk, emb) = row?;
            let score = cosine_similarity(query, &emb);
            out.push(VectorHit { doc_id, chunk, score });
        }
        out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(limit);
        Ok(out)
    }
}

pub fn chunk_text(input: &str, max_chars: usize) -> Vec<String> {
    if input.is_empty() { return vec![]; }
    let mut out = Vec::new();
    let mut buf = String::new();
    for line in input.lines() {
        if buf.len() + line.len() + 1 > max_chars && !buf.is_empty() {
            out.push(buf.trim().to_string());
            buf.clear();
        }
        buf.push_str(line);
        buf.push('\n');
    }
    if !buf.trim().is_empty() {
        out.push(buf.trim().to_string());
    }
    out
}

pub async fn embed_text_openai(api_key: &str, text: &str) -> Result<Vec<f32>> {
    #[derive(Deserialize)]
    struct EmbeddingItem { embedding: Vec<f32> }
    #[derive(Deserialize)]
    struct EmbeddingResponse { data: Vec<EmbeddingItem> }

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.openai.com/v1/embeddings")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": "text-embedding-3-small",
            "input": text,
        }))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("embedding failed: {}", resp.status());
    }

    let parsed: EmbeddingResponse = resp.json().await?;
    parsed
        .data
        .into_iter()
        .next()
        .map(|x| x.embedding)
        .ok_or_else(|| anyhow::anyhow!("no embedding returned"))
}

pub struct RagPipeline {
    store: VectorStore,
}

impl RagPipeline {
    pub fn new(store: VectorStore) -> Self { Self { store } }

    pub fn ingest_with_embeddings(&self, doc_id: &str, text: &str, embeddings: Vec<Vec<f32>>) -> Result<()> {
        let chunks = chunk_text(text, 400);
        for (chunk, emb) in chunks.iter().zip(embeddings.iter()) {
            self.store.insert_embedding(doc_id, chunk, emb)?;
        }
        Ok(())
    }

    pub fn search_with_embedding(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<VectorHit>> {
        self.store.semantic_search(query_embedding, limit)
    }
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for f in v {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

fn from_blob(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 { return 0.0; }
    let mut dot = 0.0;
    let mut na = 0.0;
    let mut nb = 0.0;
    for i in 0..len {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na.sqrt() * nb.sqrt()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_basic() {
        let a = vec![1.0, 0.0];
        let b = vec![1.0, 0.0];
        let c = vec![0.0, 1.0];
        assert!(cosine_similarity(&a, &b) > 0.99);
        assert!(cosine_similarity(&a, &c) < 0.01);
    }

    #[test]
    fn store_and_search() {
        let tmp = tempfile::tempdir().unwrap();
        let store = VectorStore::open(&tmp.path().join("v.db")).unwrap();
        store.insert_embedding("doc1", "hello world", &[1.0, 0.0]).unwrap();
        store.insert_embedding("doc2", "goodbye", &[0.0, 1.0]).unwrap();
        let hits = store.semantic_search(&[0.9, 0.1], 1).unwrap();
        assert_eq!(hits[0].doc_id, "doc1");
    }

    #[test]
    fn chunks_text() {
        let chunks = chunk_text("a\nb\nc", 3);
        assert!(!chunks.is_empty());
    }
}
