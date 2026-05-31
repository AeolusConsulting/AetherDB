use std::sync::Arc;

use aetherdb_domain::{DocumentId, Entity, EntityId, EntityType, Relationship, RelationshipId};
use aetherdb_events::{EntitiesExtracted, ExtractionFailed};
use aetherdb_storage::LibsqlStorage;

use crate::provider::{LlmProvider, LlmProviderError};

pub struct Worker {
    pub(crate) provider: Arc<dyn LlmProvider>,
    pub(crate) storage: Arc<LibsqlStorage>,
}

pub struct ProcessResult {
    pub created: Vec<EntitiesExtracted>,
    pub dlq: Vec<ExtractionFailed>,
}

impl Worker {
    pub fn new(provider: Arc<dyn LlmProvider>, storage: Arc<LibsqlStorage>) -> Self {
        Worker { provider, storage }
    }

    pub async fn process_document(&self, doc_id: DocumentId, content: &str) -> ProcessResult {
        let model = self.provider.model_name().to_string();
        let max_attempts = 3u32;
        let mut attempt = 0u32;

        let result: Result<_, ()> = loop {
            attempt += 1;
            let start = std::time::Instant::now();
            match self.provider.extract_entities(content).await {
                Ok(extraction) => break Ok((extraction, start.elapsed())),
                Err(LlmProviderError::Retryable(e)) => {
                    if attempt >= max_attempts {
                        let now_ms = chrono::Utc::now().timestamp_millis();
                        return ProcessResult {
                            created: vec![],
                            dlq: vec![ExtractionFailed {
                                document_id: doc_id,
                                model,
                                attempts: attempt,
                                last_error: e,
                                first_attempt_at: now_ms,
                                last_attempt_at: now_ms,
                            }],
                        };
                    }
                    let backoff = std::time::Duration::from_millis(100 * 2u64.pow(attempt - 1));
                    tokio::time::sleep(backoff).await;
                }
                Err(LlmProviderError::Permanent(e) | LlmProviderError::ParseError(e)) => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    return ProcessResult {
                        created: vec![],
                        dlq: vec![ExtractionFailed {
                            document_id: doc_id,
                            model,
                            attempts: attempt,
                            last_error: e,
                            first_attempt_at: now_ms,
                            last_attempt_at: now_ms,
                        }],
                    };
                }
            }
        };

        let Ok((extraction, duration)) = result else {
            return ProcessResult {
                created: vec![],
                dlq: vec![],
            };
        };

        let now = chrono::Utc::now();
        let mut entity_map = std::collections::HashMap::new();
        let mut entities = Vec::new();

        for extracted in &extraction.entities {
            let entity_id = EntityId(uuid::Uuid::now_v7());
            entity_map.insert(extracted.name.clone(), entity_id);
            entities.push(Entity {
                id: entity_id,
                name: extracted.name.clone(),
                entity_type: EntityType::from_str_lossy(&extracted.entity_type),
                document_id: doc_id,
                created_at: now,
            });
        }

        if let Err(e) = self.storage.entity_repo().insert_batch(&entities).await {
            tracing::error!("failed to insert entities: {e}");
            return ProcessResult {
                created: vec![],
                dlq: vec![],
            };
        }

        let mut relationships = Vec::new();
        for extracted in &extraction.relationships {
            let source_id = entity_map.get(&extracted.source);
            let target_id = entity_map.get(&extracted.target);
            if let (Some(&src), Some(&tgt)) = (source_id, target_id) {
                relationships.push(Relationship {
                    id: RelationshipId(uuid::Uuid::now_v7()),
                    source_entity_id: src,
                    target_entity_id: tgt,
                    relationship_type: extracted.relationship_type.clone(),
                    weight: extracted.weight.unwrap_or(1.0),
                    document_id: doc_id,
                    created_at: now,
                });
            }
        }

        if !relationships.is_empty() {
            if let Err(e) = self
                .storage
                .relationship_repo()
                .insert_batch(&relationships)
                .await
            {
                tracing::error!("failed to insert relationships: {e}");
            }
        }

        ProcessResult {
            created: vec![EntitiesExtracted {
                document_id: doc_id,
                entity_count: entities.len() as u32,
                relationship_count: relationships.len() as u32,
                model,
                duration_ms: duration.as_millis() as u64,
            }],
            dlq: vec![],
        }
    }
}
