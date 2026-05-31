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

#[derive(Debug, thiserror::Error)]
pub enum HandlerError {
    #[error("retryable: {0}")]
    Retryable(String),
    #[error("permanent: {0}")]
    Permanent(String),
}
