use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{DocumentId, EntityId, EntityType, RelationshipId};

#[derive(Debug, Deserialize)]
pub struct CreateDocumentRequest {
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDocumentRequest {
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct CreateDocumentResponse {
    pub id: DocumentId,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct DocumentDto {
    pub id: DocumentId,
    pub content: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: u32,
    #[serde(default)]
    pub min_score: Option<f32>,
}

fn default_top_k() -> u32 {
    10
}

#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub results: Vec<SearchHit>,
    pub pending_embeddings: u64,
}

#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub document_id: DocumentId,
    pub score: f32,
    pub content_preview: String,
}

// --- Graph API types ---

#[derive(Debug, Serialize)]
pub struct EntityDto {
    pub id: EntityId,
    pub name: String,
    pub entity_type: EntityType,
    pub document_id: DocumentId,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct RelationshipDto {
    pub id: RelationshipId,
    pub source_entity_id: EntityId,
    pub target_entity_id: EntityId,
    pub relationship_type: String,
    pub weight: f32,
    pub document_id: DocumentId,
}

#[derive(Debug, Serialize)]
pub struct EntitiesResponse {
    pub entities: Vec<EntityDto>,
}

#[derive(Debug, Serialize)]
pub struct RelatedDocumentsResponse {
    pub documents: Vec<RelatedDocumentHit>,
}

#[derive(Debug, Serialize)]
pub struct RelatedDocumentHit {
    pub document_id: DocumentId,
    pub entity_name: String,
    pub hop_count: u32,
}

// --- Hybrid search ---

#[derive(Debug, Deserialize)]
pub struct HybridSearchRequest {
    pub query: String,
    #[serde(default = "default_top_k")]
    pub top_k: u32,
    #[serde(default)]
    pub min_score: Option<f32>,
    #[serde(default = "default_graph_depth")]
    pub graph_depth: u32,
    #[serde(default = "default_graph_weight")]
    pub graph_weight: f32,
}

fn default_graph_depth() -> u32 {
    2
}

fn default_graph_weight() -> f32 {
    0.3
}

#[derive(Debug, Serialize)]
pub struct HybridSearchResponse {
    pub results: Vec<HybridSearchHit>,
    pub pending_embeddings: u64,
}

#[derive(Debug, Serialize)]
pub struct HybridSearchHit {
    pub document_id: DocumentId,
    pub score: f32,
    pub vector_score: Option<f32>,
    pub graph_score: Option<f32>,
    pub content_preview: String,
}
