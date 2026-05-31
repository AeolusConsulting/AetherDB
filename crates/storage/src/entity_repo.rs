use std::time::Duration;

use aetherdb_domain::{DocumentId, Entity, EntityId, EntityType, StorageError};

use crate::EntityRepo;

impl EntityRepo<'_> {
    pub async fn insert_batch(&self, entities: &[Entity]) -> Result<(), StorageError> {
        for e in entities {
            let created_at = e.created_at.timestamp_millis();
            self.conn
                .execute(
                    "INSERT INTO entities (id, name, entity_type, document_id, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    libsql::params![
                        e.id.0.to_string(),
                        e.name.clone(),
                        e.entity_type.to_string(),
                        e.document_id.0.to_string(),
                        created_at,
                    ],
                )
                .await
                .map_err(|e| StorageError::Query(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn get_by_document(&self, doc_id: &DocumentId) -> Result<Vec<Entity>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, name, entity_type, document_id, created_at
                 FROM entities WHERE document_id = ?1",
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
            result.push(row_to_entity(&row)?);
        }
        Ok(result)
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Vec<Entity>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, name, entity_type, document_id, created_at
                 FROM entities WHERE name = ?1 COLLATE NOCASE",
                libsql::params![name.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let mut result = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?
        {
            result.push(row_to_entity(&row)?);
        }
        Ok(result)
    }

    pub async fn delete_by_document(&self, doc_id: &DocumentId) -> Result<u64, StorageError> {
        let changes = self
            .conn
            .execute(
                "DELETE FROM entities WHERE document_id = ?1",
                libsql::params![doc_id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(changes)
    }

    pub async fn count_recent_without_extraction(
        &self,
        within: Duration,
    ) -> Result<u64, StorageError> {
        let cutoff = chrono::Utc::now().timestamp_millis() - within.as_millis() as i64;
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM documents d
                 LEFT JOIN entities e ON d.id = e.document_id
                 WHERE d.created_at >= ?1 AND e.document_id IS NULL",
                libsql::params![cutoff],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        match row {
            Some(r) => {
                let count: i64 = r.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
                Ok(count as u64)
            }
            None => Ok(0),
        }
    }
}

fn row_to_entity(row: &libsql::Row) -> Result<Entity, StorageError> {
    let id_str: String = row.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
    let name: String = row.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
    let entity_type_str: String = row.get(2).map_err(|e| StorageError::Query(e.to_string()))?;
    let doc_id_str: String = row.get(3).map_err(|e| StorageError::Query(e.to_string()))?;
    let created_ms: i64 = row.get(4).map_err(|e| StorageError::Query(e.to_string()))?;

    let id = uuid::Uuid::parse_str(&id_str).map_err(|e| StorageError::Query(e.to_string()))?;
    let doc_id =
        uuid::Uuid::parse_str(&doc_id_str).map_err(|e| StorageError::Query(e.to_string()))?;
    let created_at = chrono::DateTime::from_timestamp_millis(created_ms).unwrap_or_default();

    Ok(Entity {
        id: EntityId(id),
        name,
        entity_type: EntityType::from_str_lossy(&entity_type_str),
        document_id: DocumentId(doc_id),
        created_at,
    })
}
