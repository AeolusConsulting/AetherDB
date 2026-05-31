use std::collections::HashMap;

use aetherdb_domain::{ContentHash, Document, DocumentId};
use sha2::{Digest, Sha256};

use crate::error::EdgeError;
use crate::search::{SearchResult, brute_force_search};
use crate::sync::{SyncDocument, SyncEmbedding, SyncPayload, SyncStats};

pub struct EdgeStore {
    documents: HashMap<DocumentId, Document>,
    embeddings: HashMap<DocumentId, Vec<f32>>,
    embed_dim: u16,
}

impl EdgeStore {
    pub fn new(embed_dim: u16) -> Self {
        EdgeStore {
            documents: HashMap::new(),
            embeddings: HashMap::new(),
            embed_dim,
        }
    }

    pub fn insert_document(
        &mut self,
        content: String,
        metadata: serde_json::Value,
    ) -> Result<DocumentId, EdgeError> {
        if content.is_empty() {
            return Err(EdgeError::Validation("content must not be empty".into()));
        }
        let id = DocumentId(uuid::Uuid::now_v7());
        let now = chrono::Utc::now();

        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();

        let doc = Document {
            id,
            content,
            metadata,
            content_hash: ContentHash(hash),
            created_at: now,
            updated_at: now,
        };
        self.documents.insert(id, doc);
        Ok(id)
    }

    pub fn insert_document_with_embedding(
        &mut self,
        content: String,
        metadata: serde_json::Value,
        embedding: Vec<f32>,
    ) -> Result<DocumentId, EdgeError> {
        if embedding.len() != self.embed_dim as usize {
            return Err(EdgeError::DimensionMismatch {
                expected: self.embed_dim as usize,
                got: embedding.len(),
            });
        }
        let id = self.insert_document(content, metadata)?;
        self.embeddings.insert(id, embedding);
        Ok(id)
    }

    pub fn set_embedding(&mut self, id: &DocumentId, embedding: Vec<f32>) -> Result<(), EdgeError> {
        if !self.documents.contains_key(id) {
            return Err(EdgeError::NotFound(*id));
        }
        if embedding.len() != self.embed_dim as usize {
            return Err(EdgeError::DimensionMismatch {
                expected: self.embed_dim as usize,
                got: embedding.len(),
            });
        }
        self.embeddings.insert(*id, embedding);
        Ok(())
    }

    pub fn get_document(&self, id: &DocumentId) -> Option<&Document> {
        self.documents.get(id)
    }

    pub fn delete_document(&mut self, id: &DocumentId) -> bool {
        self.embeddings.remove(id);
        self.documents.remove(id).is_some()
    }

    pub fn search_similar(
        &self,
        query: &[f32],
        top_k: usize,
    ) -> Result<Vec<SearchResult>, EdgeError> {
        if query.len() != self.embed_dim as usize {
            return Err(EdgeError::DimensionMismatch {
                expected: self.embed_dim as usize,
                got: query.len(),
            });
        }
        if self.embeddings.is_empty() {
            return Ok(Vec::new());
        }

        let pairs: Vec<(DocumentId, &[f32])> = self
            .embeddings
            .iter()
            .map(|(id, vec)| (*id, vec.as_slice()))
            .collect();

        Ok(brute_force_search(query, &pairs, top_k))
    }

    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    pub fn embedding_count(&self) -> usize {
        self.embeddings.len()
    }

    pub fn export_for_sync(&self) -> Result<SyncPayload, EdgeError> {
        let documents: Vec<SyncDocument> = self
            .documents
            .values()
            .map(|d| SyncDocument {
                id: d.id,
                content: d.content.clone(),
                metadata: d.metadata.clone(),
                content_hash: hex::encode(&d.content_hash.0),
                created_at: d.created_at.timestamp_millis(),
                updated_at: d.updated_at.timestamp_millis(),
            })
            .collect();

        let embeddings: Vec<SyncEmbedding> = self
            .embeddings
            .iter()
            .map(|(id, vec)| SyncEmbedding {
                document_id: *id,
                vector: vec.clone(),
            })
            .collect();

        Ok(SyncPayload {
            version: 1,
            documents,
            embeddings,
        })
    }

    pub fn import_from_sync(&mut self, payload: &SyncPayload) -> Result<SyncStats, EdgeError> {
        let mut stats = SyncStats::default();

        for doc in &payload.documents {
            if self.documents.contains_key(&doc.id) {
                stats.documents_skipped += 1;
                continue;
            }

            let hash_bytes = hex::decode(&doc.content_hash)
                .map_err(|e| EdgeError::Serialization(e.to_string()))?;
            let mut hash = [0u8; 32];
            let len = hash_bytes.len().min(32);
            hash[..len].copy_from_slice(&hash_bytes[..len]);

            let created_at =
                chrono::DateTime::from_timestamp_millis(doc.created_at).unwrap_or_default();
            let updated_at =
                chrono::DateTime::from_timestamp_millis(doc.updated_at).unwrap_or_default();

            let document = Document {
                id: doc.id,
                content: doc.content.clone(),
                metadata: doc.metadata.clone(),
                content_hash: ContentHash(hash),
                created_at,
                updated_at,
            };
            self.documents.insert(doc.id, document);
            stats.documents_imported += 1;
        }

        for emb in &payload.embeddings {
            if self.documents.contains_key(&emb.document_id) {
                self.embeddings.insert(emb.document_id, emb.vector.clone());
                stats.embeddings_imported += 1;
            }
        }

        Ok(stats)
    }
}

// hex encode/decode without pulling in a separate crate
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        use std::fmt::Write;
        bytes
            .iter()
            .fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
                let _ = write!(s, "{b:02x}");
                s
            })
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if s.len() % 2 != 0 {
            return Err("odd length hex string".into());
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }
}
