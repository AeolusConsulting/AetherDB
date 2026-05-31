use aetherdb_domain::DocumentId;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("clock skew too large: local={local_ms}, remote={remote_ms}")]
    ClockSkew { local_ms: i64, remote_ms: i64 },
    #[error("document id mismatch: {local} vs {remote}")]
    IdMismatch {
        local: DocumentId,
        remote: DocumentId,
    },
    #[error("invalid change: {0}")]
    InvalidChange(String),
}
