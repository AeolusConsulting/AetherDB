use aetherdb_domain::EmbeddingModel;
use async_trait::async_trait;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("retryable: {0}")]
    Retryable(String),
    #[error("dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch { expected: u16, got: u16 },
    #[error("permanent: {0}")]
    Permanent(String),
}

#[async_trait]
pub trait EmbeddingProvider: Send + Sync + 'static {
    fn model(&self) -> &EmbeddingModel;
    fn dim(&self) -> u16;
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderError>;
}
