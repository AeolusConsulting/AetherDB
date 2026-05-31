use std::time::Duration;

use async_trait::async_trait;

use crate::{
    CachedResponse, Document, DocumentId, Embedding, EmbeddingModel, Entity, EntityId,
    Relationship, StorageError,
};

#[async_trait]
pub trait DocumentRepository: Send + Sync + 'static {
    async fn insert(&self, doc: &Document) -> Result<(), StorageError>;
    async fn get(&self, id: &DocumentId) -> Result<Option<Document>, StorageError>;
    async fn delete(&self, id: &DocumentId) -> Result<bool, StorageError>;
    async fn update(&self, doc: &Document) -> Result<bool, StorageError>;
    async fn count_recent_without_embedding(&self, within: Duration) -> Result<u64, StorageError>;
}

#[async_trait]
pub trait EmbeddingRepository: Send + Sync + 'static {
    async fn upsert(&self, e: &Embedding) -> Result<(), StorageError>;
    async fn exists(
        &self,
        doc_id: &DocumentId,
        model: &EmbeddingModel,
    ) -> Result<bool, StorageError>;
}

#[async_trait]
pub trait IdempotencyRepository: Send + Sync + 'static {
    async fn get(&self, key: &str) -> Result<Option<CachedResponse>, StorageError>;
    async fn put(
        &self,
        key: &str,
        response: &CachedResponse,
        ttl: Duration,
    ) -> Result<(), StorageError>;
    async fn sweep_expired(&self) -> Result<u64, StorageError>;
}

#[async_trait]
pub trait EntityRepository: Send + Sync + 'static {
    async fn insert_batch(&self, entities: &[Entity]) -> Result<(), StorageError>;
    async fn get_by_document(&self, doc_id: &DocumentId) -> Result<Vec<Entity>, StorageError>;
    async fn find_by_name(&self, name: &str) -> Result<Vec<Entity>, StorageError>;
    async fn delete_by_document(&self, doc_id: &DocumentId) -> Result<u64, StorageError>;
    async fn count_recent_without_extraction(&self, within: Duration) -> Result<u64, StorageError>;
}

#[async_trait]
pub trait RelationshipRepository: Send + Sync + 'static {
    async fn insert_batch(&self, relationships: &[Relationship]) -> Result<(), StorageError>;
    async fn get_by_entity(&self, entity_id: &EntityId) -> Result<Vec<Relationship>, StorageError>;
    async fn get_by_document(&self, doc_id: &DocumentId)
    -> Result<Vec<Relationship>, StorageError>;
    async fn delete_by_document(&self, doc_id: &DocumentId) -> Result<u64, StorageError>;
}
