use aetherdb_domain::{DocumentId, Embedding, EmbeddingModel, StorageError};

use crate::EmbeddingRepo;

impl EmbeddingRepo<'_> {
    pub async fn upsert(&self, e: &Embedding) -> Result<(), StorageError> {
        let created_at = e.created_at.timestamp_millis();
        let vec_str = format!(
            "[{}]",
            e.vector
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        self.conn
            .execute(
                "INSERT OR REPLACE INTO document_embeddings
                     (document_id, embedding, model, dim, created_at)
                 VALUES (?1, vector32(?2), ?3, ?4, ?5)",
                libsql::params![
                    e.document_id.0.to_string(),
                    vec_str,
                    e.model.0.clone(),
                    e.dim as i64,
                    created_at,
                ],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }

    pub async fn exists(
        &self,
        doc_id: &DocumentId,
        model: &EmbeddingModel,
    ) -> Result<bool, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT 1 FROM document_embeddings WHERE document_id = ?1 AND model = ?2",
                libsql::params![doc_id.0.to_string(), model.0.clone()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        Ok(row.is_some())
    }
}
