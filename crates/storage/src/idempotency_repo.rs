use std::time::Duration;

use aetherdb_domain::{CachedResponse, StorageError};

use crate::IdempotencyRepo;

impl IdempotencyRepo<'_> {
    pub async fn get(&self, key: &str) -> Result<Option<CachedResponse>, StorageError> {
        let now = chrono::Utc::now().timestamp_millis();
        let mut rows = self
            .conn
            .query(
                "SELECT response, status_code FROM idempotency_keys
                 WHERE key = ?1 AND expires_at > ?2",
                libsql::params![key.to_string(), now],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        let row = rows
            .next()
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;

        match row {
            None => Ok(None),
            Some(r) => {
                let body: String = r.get(0).map_err(|e| StorageError::Query(e.to_string()))?;
                let status: i64 = r.get(1).map_err(|e| StorageError::Query(e.to_string()))?;
                Ok(Some(CachedResponse {
                    status_code: status as u16,
                    body,
                }))
            }
        }
    }

    pub async fn put(
        &self,
        key: &str,
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), StorageError> {
        let now = chrono::Utc::now().timestamp_millis();
        let expires_at = now + ttl.as_millis() as i64;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO idempotency_keys
                     (key, response, status_code, created_at, expires_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                libsql::params![
                    key.to_string(),
                    response.body.clone(),
                    response.status_code as i64,
                    now,
                    expires_at,
                ],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(())
    }

    pub async fn sweep_expired(&self) -> Result<u64, StorageError> {
        let now = chrono::Utc::now().timestamp_millis();
        let changes = self
            .conn
            .execute(
                "DELETE FROM idempotency_keys WHERE expires_at <= ?1",
                libsql::params![now],
            )
            .await
            .map_err(|e| StorageError::Query(e.to_string()))?;
        Ok(changes)
    }
}
