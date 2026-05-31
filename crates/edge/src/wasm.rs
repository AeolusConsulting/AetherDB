use wasm_bindgen::prelude::*;

use crate::store::EdgeStore;

#[wasm_bindgen]
pub struct WasmEdgeStore {
    inner: EdgeStore,
}

#[wasm_bindgen]
impl WasmEdgeStore {
    #[wasm_bindgen(constructor)]
    pub fn new(embed_dim: u16) -> Self {
        WasmEdgeStore {
            inner: EdgeStore::new(embed_dim),
        }
    }

    pub fn insert_document(
        &mut self,
        content: &str,
        metadata_json: &str,
    ) -> Result<String, JsError> {
        let metadata: serde_json::Value =
            serde_json::from_str(metadata_json).map_err(|e| JsError::new(&e.to_string()))?;
        let id = self
            .inner
            .insert_document(content.to_string(), metadata)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(id.0.to_string())
    }

    pub fn insert_document_with_embedding(
        &mut self,
        content: &str,
        metadata_json: &str,
        embedding_json: &str,
    ) -> Result<String, JsError> {
        let metadata: serde_json::Value =
            serde_json::from_str(metadata_json).map_err(|e| JsError::new(&e.to_string()))?;
        let embedding: Vec<f32> =
            serde_json::from_str(embedding_json).map_err(|e| JsError::new(&e.to_string()))?;
        let id = self
            .inner
            .insert_document_with_embedding(content.to_string(), metadata, embedding)
            .map_err(|e| JsError::new(&e.to_string()))?;
        Ok(id.0.to_string())
    }

    pub fn get_document(&self, id: &str) -> Result<String, JsError> {
        let uid = uuid::Uuid::parse_str(id).map_err(|e| JsError::new(&e.to_string()))?;
        let doc_id = aetherdb_domain::DocumentId(uid);
        match self.inner.get_document(&doc_id) {
            Some(doc) => {
                let dto = serde_json::json!({
                    "id": doc.id,
                    "content": doc.content,
                    "metadata": doc.metadata,
                });
                serde_json::to_string(&dto).map_err(|e| JsError::new(&e.to_string()))
            }
            None => Err(JsError::new("document not found")),
        }
    }

    pub fn delete_document(&mut self, id: &str) -> Result<bool, JsError> {
        let uid = uuid::Uuid::parse_str(id).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(self
            .inner
            .delete_document(&aetherdb_domain::DocumentId(uid)))
    }

    pub fn search_similar(&self, query_json: &str, top_k: usize) -> Result<String, JsError> {
        let query: Vec<f32> =
            serde_json::from_str(query_json).map_err(|e| JsError::new(&e.to_string()))?;
        let results = self
            .inner
            .search_similar(&query, top_k)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let out: Vec<serde_json::Value> = results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "document_id": r.document_id.0.to_string(),
                    "score": r.score,
                    "distance": r.distance,
                })
            })
            .collect();
        serde_json::to_string(&out).map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn document_count(&self) -> usize {
        self.inner.document_count()
    }

    pub fn embedding_count(&self) -> usize {
        self.inner.embedding_count()
    }

    pub fn export_for_sync(&self) -> Result<String, JsError> {
        let payload = self
            .inner
            .export_for_sync()
            .map_err(|e| JsError::new(&e.to_string()))?;
        serde_json::to_string(&payload).map_err(|e| JsError::new(&e.to_string()))
    }

    pub fn import_from_sync(&mut self, payload_json: &str) -> Result<String, JsError> {
        let payload: crate::sync::SyncPayload =
            serde_json::from_str(payload_json).map_err(|e| JsError::new(&e.to_string()))?;
        let stats = self
            .inner
            .import_from_sync(&payload)
            .map_err(|e| JsError::new(&e.to_string()))?;
        let out = serde_json::json!({
            "documents_imported": stats.documents_imported,
            "embeddings_imported": stats.embeddings_imported,
            "documents_skipped": stats.documents_skipped,
        });
        serde_json::to_string(&out).map_err(|e| JsError::new(&e.to_string()))
    }
}
