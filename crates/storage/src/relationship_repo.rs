use aetherdb_domain::{DocumentId, EntityId, Relationship, RelationshipId, StorageError};

use crate::RelationshipRepo;

impl RelationshipRepo<'_> {
    pub async fn insert_batch(&self, relationships: &[Relationship]) -> Result<(), StorageError> {
        for r in relationships {
            let created_at = r.created_at.timestamp_millis();
            self.conn
                .execute(
                    "INSERT INTO relationships (id, source_entity_id, target_entity_id, relationship_type, weight, document_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    libsql::params![
                        r.id.0.to_string(),
                        r.source_entity_id.0.to_string(),
                        r.target_entity_id.0.to_string(),
                        r.relationship_type.clone(),
                        r.weight as f64,
                        r.document_id.0.to_string(),
                        created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn get_by_entity(
        &self,
        entity_id: &EntityId,
    ) -> Result<Vec<Relationship>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, source_entity_id, target_entity_id, relationship_type, weight, document_id, created_at
                 FROM relationships WHERE source_entity_id = ?1 OR target_entity_id = ?1",
                libsql::params![entity_id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let mut result = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?
        {
            result.push(row_to_relationship(&row)?);
        }
        Ok(result)
    }

    pub async fn get_by_document(
        &self,
        doc_id: &DocumentId,
    ) -> Result<Vec<Relationship>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, source_entity_id, target_entity_id, relationship_type, weight, document_id, created_at
                 FROM relationships WHERE document_id = ?1",
                libsql::params![doc_id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let mut result = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?
        {
            result.push(row_to_relationship(&row)?);
        }
        Ok(result)
    }

    pub async fn delete_by_document(&self, doc_id: &DocumentId) -> Result<u64, StorageError> {
        let changes = self
            .conn
            .execute(
                "DELETE FROM relationships WHERE document_id = ?1",
                libsql::params![doc_id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(changes)
    }
}

fn row_to_relationship(row: &libsql::Row) -> Result<Relationship, StorageError> {
    let id_str: String = row.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
    let src_str: String = row.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
    let tgt_str: String = row.get(2).map_err(|e| StorageError::Query(e.to_string()))?;
    let rel_type: String = row.get(3).map_err(|e| StorageError::Query(e.to_string()))?;
    let weight: f64 = row.get(4).map_err(|e| StorageError::Query(e.to_string()))?;
    let doc_str: String = row.get(5).map_err(|e| StorageError::Query(e.to_string()))?;
    let created_ms: i64 = row.get(6).map_err(|e| StorageError::Query(e.to_string()))?;

    let parse = |s: &str| uuid::Uuid::parse_str(s).map_err(|e| StorageError::Query(e.to_string()));

    Ok(Relationship {
        id: RelationshipId(parse(&id_str)?),
        source_entity_id: EntityId(parse(&src_str)?),
        target_entity_id: EntityId(parse(&tgt_str)?),
        relationship_type: rel_type,
        weight: weight as f32,
        document_id: DocumentId(parse(&doc_str)?),
        created_at: chrono::DateTime::from_timestamp_millis(created_ms).unwrap_or_default(),
    })
}
