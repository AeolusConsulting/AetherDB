#[derive(Debug, thiserror::Error)]
pub enum EdgeError {
    #[error("document not found: {0}")]
    NotFound(aetherdb_domain::DocumentId),
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("validation error: {0}")]
    Validation(String),
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("store is empty, no vectors indexed")]
    EmptyIndex,
}
