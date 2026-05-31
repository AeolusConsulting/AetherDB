use std::time::Duration;

use aetherdb_domain::{ContentHash, Document, DocumentId, StorageError};
use aetherdb_sync::{Change, HlcTimestamp, NodeId, VersionedDocument};

use crate::DocumentRepo;

impl DocumentRepo<'_> {
    pub async fn insert(&self, doc: &Document) -> Result<(), StorageError> {
        let created_at = doc.created_at.timestamp_millis();
        let updated_at = doc.updated_at.timestamp_millis();
        let metadata_str =
            serde_json::to_string(&doc.metadata).map_err(|e| StorageError::Query(e.to_string()))?;
        self.conn
            .execute(
                "INSERT INTO documents (id, content, metadata, content_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                libsql::params![
                    doc.id.0.to_string(),
                    doc.content.clone(),
                    metadata_str,
                    doc.content_hash.0.to_vec(),
                    created_at,
                    updated_at,
                ],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }

    pub async fn get(&self, id: &DocumentId) -> Result<Option<Document>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, content, metadata, content_hash, created_at, updated_at
                 FROM documents WHERE id = ?1",
                libsql::params![id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(row) => {
                let id_str: String = row.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
                let content: String = row.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
                let metadata_str: String =
                    row.get(2).map_err(|e| StorageError::Query(e.to_string()))?;
                let hash_blob: Vec<u8> =
                    row.get(3).map_err(|e| StorageError::Query(e.to_string()))?;
                let created_ms: i64 = row.get(4).map_err(|e| StorageError::Query(e.to_string()))?;
                let updated_ms: i64 = row.get(5).map_err(|e| StorageError::Query(e.to_string()))?;

                let uid = uuid::Uuid::parse_str(&id_str)
                    .map_err(|e| StorageError::Query(e.to_string()))?;
                let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                    .map_err(|e| StorageError::Query(e.to_string()))?;

                let mut hash = [0u8; 32];
                let len = hash_blob.len().min(32);
                hash[..len].copy_from_slice(&hash_blob[..len]);

                let created_at =
                    chrono::DateTime::from_timestamp_millis(created_ms).unwrap_or_default();
                let updated_at =
                    chrono::DateTime::from_timestamp_millis(updated_ms).unwrap_or_default();

                Ok(Some(Document {
                    id: DocumentId(uid),
                    content,
                    metadata,
                    content_hash: ContentHash(hash),
                    created_at,
                    updated_at,
                }))
            }
        }
    }

    pub async fn delete(&self, id: &DocumentId) -> Result<bool, StorageError> {
        let changes = self
            .conn
            .execute(
                "DELETE FROM documents WHERE id = ?1",
                libsql::params![id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(changes > 0)
    }

    pub async fn update(&self, doc: &Document) -> Result<bool, StorageError> {
        let updated_at = doc.updated_at.timestamp_millis();
        let metadata_str =
            serde_json::to_string(&doc.metadata).map_err(|e| StorageError::Query(e.to_string()))?;
        let changes = self
            .conn
            .execute(
                "UPDATE documents SET content = ?2, metadata = ?3, content_hash = ?4, updated_at = ?5
                 WHERE id = ?1",
                libsql::params![
                    doc.id.0.to_string(),
                    doc.content.clone(),
                    metadata_str,
                    doc.content_hash.0.to_vec(),
                    updated_at,
                ],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(changes > 0)
    }

    pub async fn count_recent_without_embedding(
        &self,
        within: Duration,
    ) -> Result<u64, StorageError> {
        let cutoff = chrono::Utc::now().timestamp_millis() - within.as_millis() as i64;
        let mut rows = self
            .conn
            .query(
                "SELECT COUNT(*) FROM documents d
                 LEFT JOIN document_embeddings de ON d.id = de.document_id
                 WHERE d.created_at >= ?1 AND de.document_id IS NULL",
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

    pub async fn upsert_with_hlc(
        &self,
        doc: &Document,
        hlc_ts: &HlcTimestamp,
        origin_node: &NodeId,
        deleted: bool,
    ) -> Result<(), StorageError> {
        let created_at = doc.created_at.timestamp_millis();
        let updated_at = doc.updated_at.timestamp_millis();
        let metadata_str =
            serde_json::to_string(&doc.metadata).map_err(|e| StorageError::Query(e.to_string()))?;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO documents
                    (id, content, metadata, content_hash, created_at, updated_at,
                     hlc_wall_ms, hlc_counter, origin_node, deleted)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                libsql::params![
                    doc.id.0.to_string(),
                    doc.content.clone(),
                    metadata_str,
                    doc.content_hash.0.to_vec(),
                    created_at,
                    updated_at,
                    hlc_ts.wall_ms,
                    hlc_ts.counter as i64,
                    origin_node.0.to_string(),
                    if deleted { 1i64 } else { 0i64 },
                ],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }

    pub async fn get_changes_since(
        &self,
        since: &HlcTimestamp,
    ) -> Result<Vec<Change>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, content, metadata, content_hash, hlc_wall_ms, hlc_counter,
                        origin_node, deleted
                 FROM documents
                 WHERE (hlc_wall_ms > ?1) OR (hlc_wall_ms = ?1 AND hlc_counter > ?2)
                 ORDER BY hlc_wall_ms ASC, hlc_counter ASC",
                libsql::params![since.wall_ms, since.counter as i64],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let mut changes = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?
        {
            let id_str: String = row.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
            let content: String = row.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
            let metadata_str: String =
                row.get(2).map_err(|e| StorageError::Query(e.to_string()))?;
            let hash_blob: Vec<u8> = row.get(3).map_err(|e| StorageError::Query(e.to_string()))?;
            let wall_ms: i64 = row.get(4).map_err(|e| StorageError::Query(e.to_string()))?;
            let counter: i64 = row.get(5).map_err(|e| StorageError::Query(e.to_string()))?;
            let origin_str: String = row.get(6).map_err(|e| StorageError::Query(e.to_string()))?;
            let deleted_int: i64 = row.get(7).map_err(|e| StorageError::Query(e.to_string()))?;

            let uid =
                uuid::Uuid::parse_str(&id_str).map_err(|e| StorageError::Query(e.to_string()))?;
            let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                .map_err(|e| StorageError::Query(e.to_string()))?;
            let origin_node = if origin_str.is_empty() {
                NodeId(uuid::Uuid::nil())
            } else {
                NodeId(
                    uuid::Uuid::parse_str(&origin_str)
                        .map_err(|e| StorageError::Query(e.to_string()))?,
                )
            };

            let hash_hex: String = hash_blob.iter().fold(String::new(), |mut s, b| {
                use std::fmt::Write;
                let _ = write!(s, "{b:02x}");
                s
            });

            changes.push(Change {
                document_id: DocumentId(uid),
                content,
                metadata,
                content_hash: hash_hex,
                hlc_ts: HlcTimestamp {
                    wall_ms,
                    counter: counter as u32,
                },
                origin_node,
                deleted: deleted_int != 0,
            });
        }
        Ok(changes)
    }

    pub async fn get_with_hlc(
        &self,
        id: &DocumentId,
    ) -> Result<Option<VersionedDocument>, StorageError> {
        let mut rows = self
            .conn
            .query(
                "SELECT id, content, metadata, content_hash, created_at, updated_at,
                        hlc_wall_ms, hlc_counter, origin_node, deleted
                 FROM documents WHERE id = ?1",
                libsql::params![id.0.to_string()],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(row) => {
                let id_str: String = row.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
                let content: String = row.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
                let metadata_str: String =
                    row.get(2).map_err(|e| StorageError::Query(e.to_string()))?;
                let hash_blob: Vec<u8> =
                    row.get(3).map_err(|e| StorageError::Query(e.to_string()))?;
                let created_ms: i64 = row.get(4).map_err(|e| StorageError::Query(e.to_string()))?;
                let updated_ms: i64 = row.get(5).map_err(|e| StorageError::Query(e.to_string()))?;
                let wall_ms: i64 = row.get(6).map_err(|e| StorageError::Query(e.to_string()))?;
                let counter: i64 = row.get(7).map_err(|e| StorageError::Query(e.to_string()))?;
                let origin_str: String =
                    row.get(8).map_err(|e| StorageError::Query(e.to_string()))?;
                let deleted_int: i64 =
                    row.get(9).map_err(|e| StorageError::Query(e.to_string()))?;

                let uid = uuid::Uuid::parse_str(&id_str)
                    .map_err(|e| StorageError::Query(e.to_string()))?;
                let metadata: serde_json::Value = serde_json::from_str(&metadata_str)
                    .map_err(|e| StorageError::Query(e.to_string()))?;
                let mut hash = [0u8; 32];
                let len = hash_blob.len().min(32);
                hash[..len].copy_from_slice(&hash_blob[..len]);
                let origin_node = if origin_str.is_empty() {
                    NodeId(uuid::Uuid::nil())
                } else {
                    NodeId(
                        uuid::Uuid::parse_str(&origin_str)
                            .map_err(|e| StorageError::Query(e.to_string()))?,
                    )
                };

                Ok(Some(VersionedDocument {
                    document: Document {
                        id: DocumentId(uid),
                        content,
                        metadata,
                        content_hash: ContentHash(hash),
                        created_at: chrono::DateTime::from_timestamp_millis(created_ms)
                            .unwrap_or_default(),
                        updated_at: chrono::DateTime::from_timestamp_millis(updated_ms)
                            .unwrap_or_default(),
                    },
                    hlc_ts: HlcTimestamp {
                        wall_ms,
                        counter: counter as u32,
                    },
                    origin_node,
                    deleted: deleted_int != 0,
                }))
            }
        }
    }
}
