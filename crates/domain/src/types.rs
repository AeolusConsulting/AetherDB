use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct DocumentId(pub Uuid);

#[derive(Debug, Clone)]
pub struct ContentHash(pub [u8; 32]);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingModel(pub String);

impl std::fmt::Display for DocumentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::fmt::Display for EmbeddingModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentId,
    pub content: String,
    pub metadata: serde_json::Value,
    pub content_hash: ContentHash,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Embedding {
    pub document_id: DocumentId,
    pub vector: Vec<f32>,
    pub model: EmbeddingModel,
    pub dim: u16,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CachedResponse {
    pub status_code: u16,
    pub body: String,
}

// --- Graph types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EntityId(pub Uuid);

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Person,
    Organization,
    Concept,
    Location,
    Event,
    Technology,
    Other,
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Person => "person",
            Self::Organization => "organization",
            Self::Concept => "concept",
            Self::Location => "location",
            Self::Event => "event",
            Self::Technology => "technology",
            Self::Other => "other",
        };
        write!(f, "{s}")
    }
}

impl EntityType {
    pub fn from_str_lossy(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "person" => Self::Person,
            "organization" | "org" => Self::Organization,
            "concept" => Self::Concept,
            "location" | "place" => Self::Location,
            "event" => Self::Event,
            "technology" | "tech" => Self::Technology,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Entity {
    pub id: EntityId,
    pub name: String,
    pub entity_type: EntityType,
    pub document_id: DocumentId,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RelationshipId(pub Uuid);

impl std::fmt::Display for RelationshipId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: RelationshipId,
    pub source_entity_id: EntityId,
    pub target_entity_id: EntityId,
    pub relationship_type: String,
    pub weight: f32,
    pub document_id: DocumentId,
    pub created_at: DateTime<Utc>,
}
