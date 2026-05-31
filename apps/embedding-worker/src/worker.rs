use std::sync::Arc;
use std::time::Duration;

use aetherdb_domain::{DocumentId, Embedding};
use aetherdb_events::{EmbeddingCreated, EmbeddingFailed};
use aetherdb_storage::LibsqlStorage;

use crate::provider::{EmbeddingProvider, ProviderError};

pub struct Worker {
    pub(crate) provider: Arc<dyn EmbeddingProvider>,
    pub(crate) storage: Arc<LibsqlStorage>,
    pub(crate) embed_dim: u16,
}

pub struct ProcessResult {
    pub created: Vec<EmbeddingCreated>,
    pub dlq: Vec<EmbeddingFailed>,
}

impl Worker {
    pub fn new(
        provider: Arc<dyn EmbeddingProvider>,
        storage: Arc<LibsqlStorage>,
        embed_dim: u16,
    ) -> Self {
        Worker {
            provider,
            storage,
            embed_dim,
        }
    }

    pub async fn process_batch(&self, batch: &[(DocumentId, String)]) -> ProcessResult {
        let mut created = Vec::new();
        let mut dlq = Vec::new();

        let model = self.provider.model().clone();

        let mut to_embed: Vec<(DocumentId, String)> = Vec::new();
        for (doc_id, content) in batch {
            let exists = self
                .storage
                .embedding_repo()
                .exists(doc_id, &model)
                .await
                .unwrap_or(false);
            if !exists {
                to_embed.push((*doc_id, content.clone()));
            }
        }

        if to_embed.is_empty() {
            return ProcessResult { created, dlq };
        }

        let texts: Vec<String> = to_embed.iter().map(|(_, c)| c.clone()).collect();
        let max_attempts = 3u32;

        let mut attempt = 0u32;
        let result = loop {
            attempt += 1;
            let start = std::time::Instant::now();
            match self.provider.embed(&texts).await {
                Ok(vectors) => break Ok((vectors, start.elapsed())),
                Err(ProviderError::DimensionMismatch { expected, got }) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    for (doc_id, _) in &to_embed {
                        dlq.push(EmbeddingFailed {
                            document_id: *doc_id,
                            model: model.0.clone(),
                            attempts: 1,
                            last_error: format!(
                                "DimensionMismatch: expected {expected}, got {got}"
                            ),
                            first_attempt_at: now_ms,
                            last_attempt_at: now_ms,
                        });
                    }
                    return ProcessResult { created, dlq };
                }
                Err(ProviderError::Permanent(e)) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    for (doc_id, _) in &to_embed {
                        dlq.push(EmbeddingFailed {
                            document_id: *doc_id,
                            model: model.0.clone(),
                            attempts: attempt,
                            last_error: e.clone(),
                            first_attempt_at: now_ms,
                            last_attempt_at: now_ms,
                        });
                    }
                    return ProcessResult { created, dlq };
                }
                Err(ProviderError::Retryable(e)) => {
                    if attempt >= max_attempts {
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        for (doc_id, _) in &to_embed {
                            dlq.push(EmbeddingFailed {
                                document_id: *doc_id,
                                model: model.0.clone(),
                                attempts: attempt,
                                last_error: e.clone(),
                                first_attempt_at: now_ms,
                                last_attempt_at: now_ms,
                            });
                        }
                        break Err(());
                    }
                    let backoff = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
            }
        };

        if let Ok((vectors, duration)) = result {
            for (i, (doc_id, _)) in to_embed.iter().enumerate() {
                if let Some(vec) = vectors.get(i) {
                    if vec.len() != self.embed_dim as usize {
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        dlq.push(EmbeddingFailed {
                            document_id: *doc_id,
                            model: model.0.clone(),
                            attempts: 1,
                            last_error: format!(
                                "DimensionMismatch: expected {}, got {}",
                                self.embed_dim,
                                vec.len()
                            ),
                            first_attempt_at: now_ms,
                            last_attempt_at: now_ms,
                        });
                        continue;
                    }

                    let embedding = Embedding {
                        document_id: *doc_id,
                        vector: vec.clone(),
                        model: model.clone(),
                        dim: self.embed_dim,
                        created_at: chrono::Utc::now(),
                    };

                    match self.storage.embedding_repo().upsert(&embedding).await {
                        Ok(()) => {
                            created.push(EmbeddingCreated {
                                document_id: *doc_id,
                                model: model.0.clone(),
                                dim: self.embed_dim,
                                duration_ms: duration.as_millis() as u64,
                            });
                        }
                        Err(_e) => {
                            tracing::error!("failed to upsert embedding for {doc_id}");
                        }
                    }
                }
            }
        }

        ProcessResult { created, dlq }
    }
}
