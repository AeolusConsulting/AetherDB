use std::sync::Arc;

use aetherdb_domain::{DocumentId, Entity, GraphError, StorageError};
use aetherdb_storage::LibsqlStorage;

pub struct GraphQuerier {
    storage: Arc<LibsqlStorage>,
}

impl GraphQuerier {
    pub fn new(storage: Arc<LibsqlStorage>) -> Self {
        GraphQuerier { storage }
    }

    pub async fn find_related_documents(
        &self,
        entity_name: &str,
        max_depth: u32,
    ) -> Result<Vec<(DocumentId, u32)>, GraphError> {
        if max_depth > 5 {
            return Err(GraphError::InvalidDepth(max_depth));
        }

        let sql = r"
            WITH RECURSIVE reachable(entity_id, hop) AS (
                SELECT id, 0 FROM entities WHERE name = ?1 COLLATE NOCASE
                UNION ALL
                SELECT
                    CASE WHEN r.source_entity_id = re.entity_id THEN r.target_entity_id
                         ELSE r.source_entity_id END,
                    re.hop + 1
                FROM reachable re
                JOIN relationships r ON r.source_entity_id = re.entity_id
                                     OR r.target_entity_id = re.entity_id
                WHERE re.hop < ?2
            )
            SELECT DISTINCT e.document_id, MIN(re.hop) as min_hop
            FROM reachable re
            JOIN entities e ON e.id = re.entity_id
            GROUP BY e.document_id
            ORDER BY min_hop ASC
        ";

        let mut rows = self
            .storage
            .conn()
            .query(
                sql,
                libsql::params![entity_name.to_string(), max_depth as i64],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?
        {
            let doc_id_str: String = row.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
            let hop: i64 = row.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
            let uid = uuid::Uuid::parse_str(&doc_id_str)
                .map_err(|e| StorageError::Query(e.to_string()))?;
            results.push((DocumentId(uid), hop as u32));
        }

        Ok(results)
    }

    pub async fn find_entities_for_query(
        &self,
        query_text: &str,
    ) -> Result<Vec<Entity>, GraphError> {
        let words: Vec<&str> = query_text.split_whitespace().collect();
        let mut all_entities = Vec::new();

        for word in words {
            if word.len() < 3 {
                continue;
            }
            let entities = self.storage.entity_repo().find_by_name(word).await?;
            all_entities.extend(entities);
        }

        Ok(all_entities)
    }
}
