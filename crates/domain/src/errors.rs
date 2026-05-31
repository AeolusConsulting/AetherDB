#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    #[error("document not found: {0}")]
    NotFound(crate::DocumentId),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("embedding dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("connection error: {0}")]
    Connection(String),
    #[error("query error: {0}")]
    Query(String),
    #[error("migration error: {0}")]
    Migration(String),
    #[error("{0}")]
    Other(String),
    #[error("sync error: {0}")]
    Sync(String),
}

impl StorageError {
    pub fn is_retryable(&self) -> bool {
        matches!(self, StorageError::Connection(_) | StorageError::Sync(_))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VectorError {
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("vector contains non-finite values")]
    NonFinite,
    #[error("top_k must be in 1..=100, got {0}")]
    InvalidTopK(usize),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("entity not found: {0}")]
    EntityNotFound(String),
    #[error("extraction failed: {0}")]
    ExtractionFailed(String),
    #[error("invalid traversal depth: {0} (max 5)")]
    InvalidDepth(u32),
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
}

#[derive(Debug, thiserror::Error)]
pub enum HnswError {
    #[error("index not ready")]
    NotReady,
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: usize, got: usize },
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("build error: {0}")]
    Build(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MessagingError {
    #[error("connection error: {0}")]
    Connection(String),
    #[error("publish error: {0}")]
    Publish(String),
    #[error("consume error: {0}")]
    Consume(String),
    #[error("{0}")]
    Other(String),
}
