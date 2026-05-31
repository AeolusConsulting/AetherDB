mod document_repo;
mod embedding_repo;
mod entity_repo;
mod idempotency_repo;
mod relationship_repo;

use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use aetherdb_domain::{Document, StorageError};
use libsql::{Connection, Database};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Local,
    Replica,
}

pub struct LibsqlStorage {
    #[allow(dead_code)]
    db: Database,
    conn: Connection,
    mode: StorageMode,
}

pub struct DocumentRepo<'a> {
    pub(crate) conn: &'a Connection,
}

pub struct EmbeddingRepo<'a> {
    pub(crate) conn: &'a Connection,
}

pub struct IdempotencyRepo<'a> {
    pub(crate) conn: &'a Connection,
}

pub struct EntityRepo<'a> {
    pub(crate) conn: &'a Connection,
}

pub struct RelationshipRepo<'a> {
    pub(crate) conn: &'a Connection,
}

pub struct Transaction {
    inner: libsql::Transaction,
}

#[derive(Debug, Default)]
pub struct SyncStats {
    pub duration: Duration,
    pub frames_synced: usize,
}

impl LibsqlStorage {
    pub async fn connect(
        url: &str,
        auth_token: Option<&str>,
        replica_local_path: Option<&str>,
        sync_interval_secs: u64,
    ) -> Result<Self, StorageError> {
        let (db, mode) = if let Some(local_path) = replica_local_path {
            let token = auth_token.unwrap_or("");
            let db =
                libsql::Builder::new_remote_replica(local_path, url.to_string(), token.to_string())
                    .read_your_writes(true)
                    .sync_interval(Duration::from_secs(sync_interval_secs))
                    .build()
                    .await
                    .map_err(|e| {
                        StorageError::Connection(format!("replica connect to {url}: {e}"))
                    })?;
            (db, StorageMode::Replica)
        } else if url.starts_with("file:") || url.contains(":memory:") {
            let db = libsql::Builder::new_local(url)
                .build()
                .await
                .map_err(|e| StorageError::Connection(e.to_string()))?;
            (db, StorageMode::Local)
        } else {
            let token = auth_token.unwrap_or("");
            let db = libsql::Builder::new_remote(url.to_string(), token.to_string())
                .build()
                .await
                .map_err(|e| StorageError::Connection(format!("remote connect to {url}: {e}")))?;
            (db, StorageMode::Local)
        };

        let conn = db
            .connect()
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        conn.execute("PRAGMA foreign_keys = ON", ())
            .await
            .map_err(|e| StorageError::Connection(e.to_string()))?;

        Ok(LibsqlStorage { db, conn, mode })
    }

    pub async fn migrate(&self) -> Result<(), StorageError> {
        self.conn
            .execute(
                "CREATE TABLE IF NOT EXISTS schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at INTEGER NOT NULL
                )",
                (),
            )
            .await
            .map_err(|e| StorageError::Migration(e.to_string()))?;

        let migrations: &[(i64, &str)] = &[
            (1, include_str!("../../../migrations/001_documents.sql")),
            (2, &migration_002()),
            (3, include_str!("../../../migrations/003_idempotency.sql")),
            (4, include_str!("../../../migrations/004_entities.sql")),
            (5, include_str!("../../../migrations/005_relationships.sql")),
            (6, include_str!("../../../migrations/006_sync_columns.sql")),
        ];

        for &(version, sql) in migrations {
            let already_applied = self
                .conn
                .query(
                    "SELECT 1 FROM schema_migrations WHERE version = ?1",
                    libsql::params![version],
                )
                .await
                .map_err(|e| StorageError::Migration(e.to_string()))?
                .next()
                .await
                .map_err(|e| StorageError::Migration(e.to_string()))?
                .is_some();

            if already_applied {
                continue;
            }

            self.conn
                .execute_batch(sql)
                .await
                .map_err(|e| StorageError::Migration(format!("migration {version}: {e}")))?;

            let now = chrono::Utc::now().timestamp_millis();
            self.conn
                .execute(
                    "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                    libsql::params![version, now],
                )
                .await
                .map_err(|e| StorageError::Migration(e.to_string()))?;
        }

        Ok(())
    }

    pub fn document_repo(&self) -> DocumentRepo<'_> {
        DocumentRepo { conn: &self.conn }
    }

    pub fn embedding_repo(&self) -> EmbeddingRepo<'_> {
        EmbeddingRepo { conn: &self.conn }
    }

    pub fn idempotency_repo(&self) -> IdempotencyRepo<'_> {
        IdempotencyRepo { conn: &self.conn }
    }

    pub fn entity_repo(&self) -> EntityRepo<'_> {
        EntityRepo { conn: &self.conn }
    }

    pub fn relationship_repo(&self) -> RelationshipRepo<'_> {
        RelationshipRepo { conn: &self.conn }
    }

    pub async fn with_transaction<F, T>(&self, f: F) -> Result<T, StorageError>
    where
        F: for<'a> FnOnce(
                &'a mut Transaction,
            )
                -> Pin<Box<dyn Future<Output = Result<T, StorageError>> + Send + 'a>>
            + Send,
    {
        let tx = self
            .conn
            .transaction()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        let mut wrapper = Transaction { inner: tx };
        let result = f(&mut wrapper).await;
        match &result {
            Ok(_) => {
                wrapper
                    .inner
                    .commit()
                    .await
                    .map_err(|e| StorageError::Query(e.to_string()))?;
            }
            Err(_) => {
                wrapper
                    .inner
                    .rollback()
                    .await
                    .map_err(|e| StorageError::Query(e.to_string()))?;
            }
        }
        result
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn mode(&self) -> StorageMode {
        self.mode
    }

    pub async fn sync(&self) -> Result<SyncStats, StorageError> {
        match self.mode {
            StorageMode::Local => Ok(SyncStats::default()),
            StorageMode::Replica => {
                let start = std::time::Instant::now();
                let replicated = self
                    .db
                    .sync()
                    .await
                    .map_err(|e| StorageError::Connection(format!("sync failed: {e}")))?;
                let duration = start.elapsed();
                Ok(SyncStats {
                    duration,
                    frames_synced: replicated.frames_synced(),
                })
            }
        }
    }

    pub async fn is_primary_reachable(&self) -> bool {
        match self.mode {
            StorageMode::Local => true,
            StorageMode::Replica => self.db.sync().await.is_ok(),
        }
    }
}

impl Transaction {
    pub async fn insert_document(&self, doc: &Document) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp_millis();
        self.inner
            .execute(
                "INSERT INTO documents (id, content, metadata, content_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                libsql::params![
                    doc.id.0.to_string(),
                    doc.content.clone(),
                    serde_json::to_string(&doc.metadata)
                        .map_err(|e| StorageError::Query(e.to_string()))?,
                    doc.content_hash.0.to_vec(),
                    now,
                    now,
                ],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }
}

fn migration_002() -> String {
    let dim_str = std::env::var("AETHERDB_EMBED_DIM").unwrap_or_else(|_| "768".to_string());
    let dim: u32 = dim_str.parse().unwrap_or(768);
    format!(
        "CREATE TABLE IF NOT EXISTS document_embeddings (
            document_id TEXT NOT NULL UNIQUE REFERENCES documents(id) ON DELETE CASCADE,
            embedding   F32_BLOB({dim}) NOT NULL,
            model       TEXT NOT NULL,
            dim         INTEGER NOT NULL,
            created_at  INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_embeddings_vector ON document_embeddings(
            libsql_vector_idx(embedding, 'metric=cosine')
        );"
    )
}
