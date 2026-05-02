use std::collections::HashMap;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChromaConfig {
    pub enabled: bool,
}

impl Default for ChromaConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SimilarResult {
    pub invoice_id: i64,
    pub similarity: f64,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ChromaError {
    #[error("vector store not enabled")]
    NotConfigured,
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn upsert_embedding(
    conn: &Connection,
    invoice_id: i64,
    embedding: &[f32],
    text: &str,
) -> Result<(), ChromaError> {
    let blob = embedding_to_blob(embedding);
    conn.execute(
        "INSERT INTO invoice_embeddings (invoice_id, embedding, text_content)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(invoice_id) DO UPDATE SET
            embedding = excluded.embedding,
            text_content = excluded.text_content,
            created_at = datetime('now')",
        params![invoice_id, blob, text],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn delete_embedding(conn: &Connection, invoice_id: i64) -> Result<(), ChromaError> {
    conn.execute(
        "DELETE FROM invoice_embeddings WHERE invoice_id = ?1",
        [invoice_id],
    )?;
    Ok(())
}

pub fn query_similar(
    conn: &Connection,
    query_embedding: &[f32],
    limit: usize,
) -> Result<Vec<SimilarResult>, ChromaError> {
    let mut stmt = conn.prepare("SELECT invoice_id, embedding FROM invoice_embeddings")?;

    let rows: Vec<(i64, Vec<u8>)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<Vec<_>, _>>()?;

    let mut scored: Vec<(i64, f64)> = rows
        .into_iter()
        .filter_map(|(invoice_id, blob)| {
            let emb = blob_to_embedding(&blob);
            let similarity = cosine_similarity(query_embedding, &emb);
            if similarity.is_finite() {
                Some((invoice_id, similarity))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(limit);

    Ok(scored
        .into_iter()
        .map(|(invoice_id, similarity)| SimilarResult {
            invoice_id,
            similarity,
            metadata: HashMap::new(),
        })
        .collect())
}

fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    let bytes: &[u8] =
        unsafe { std::slice::from_raw_parts(embedding.as_ptr() as *const u8, embedding.len() * 4) };
    bytes.to_vec()
}

fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    let len = blob.len() / 4;
    let mut vec = Vec::with_capacity(len);
    for i in 0..len {
        let bytes: [u8; 4] = blob[i * 4..(i + 1) * 4].try_into().unwrap();
        vec.push(f32::from_le_bytes(bytes));
    }
    vec
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let len = a.len().min(b.len());
    let mut dot = 0.0_f64;
    let mut norm_a = 0.0_f64;
    let mut norm_b = 0.0_f64;
    for i in 0..len {
        let av = a[i] as f64;
        let bv = b[i] as f64;
        dot += av * bv;
        norm_a += av * av;
        norm_b += bv * bv;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        0.0
    } else {
        dot / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::run_migrations;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().expect("open");
        run_migrations(&mut conn).expect("migrate");
        // Insert a test invoice for FK
        conn.execute(
            "INSERT INTO raw_files (id, sha256, md5, original_name, current_name, extension, mime_type, byte_size, storage_path)
            VALUES (1, 's', 'm', 't.jpg', 't.jpg', 'jpg', 'image/jpeg', 10, '/t.jpg')",
            [],
        ).expect("raw");
        conn.execute(
            "INSERT INTO invoices (id, raw_file_id, invoice_type, status, duplicate_status)
            VALUES (1, 1, 'test', 'recognized', 'unique')",
            [],
        )
        .expect("invoice");
        conn
    }

    #[test]
    fn round_trip_embedding_blob() {
        let original = vec![0.1_f32, -0.5, 0.8, 0.0];
        let blob = embedding_to_blob(&original);
        let recovered = blob_to_embedding(&blob);
        assert_eq!(original.len(), recovered.len());
        for (a, b) in original.iter().zip(recovered.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn cosine_same_vector_is_one() {
        let v = vec![1.0_f32, 2.0, 3.0];
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6);
    }

    #[test]
    fn upsert_and_query() {
        let conn = setup();
        let emb = vec![0.5_f32; 128];
        upsert_embedding(&conn, 1, &emb, "test invoice").expect("upsert");

        let results = query_similar(&conn, &emb, 5).expect("query");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].invoice_id, 1);
        assert!((results[0].similarity - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_delete_embedding() {
        let conn = setup();
        let emb = vec![0.5_f32; 128];
        upsert_embedding(&conn, 1, &emb, "test").expect("upsert");
        super::delete_embedding(&conn, 1).expect("delete");
        let results = query_similar(&conn, &emb, 5).expect("query");
        assert!(results.is_empty());
    }

    #[test]
    fn default_config_is_enabled() {
        let config = ChromaConfig::default();
        assert!(config.enabled);
    }
}
