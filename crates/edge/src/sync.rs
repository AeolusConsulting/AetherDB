use aetherdb_domain::DocumentId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPayload {
    pub version: u32,
    pub documents: Vec<SyncDocument>,
    pub embeddings: Vec<SyncEmbedding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncDocument {
    pub id: DocumentId,
    pub content: String,
    pub metadata: serde_json::Value,
    pub content_hash: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncEmbedding {
    pub document_id: DocumentId,
    pub vector: Vec<f32>,
}

#[derive(Debug, Default)]
pub struct SyncStats {
    pub documents_imported: usize,
    pub embeddings_imported: usize,
    pub documents_skipped: usize,
}
