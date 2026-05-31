use aetherdb_domain::DocumentId;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

pub trait Event: Send + Sync + 'static {
    const TOPIC: &'static str;
    const VERSION: u32;
    type Payload: Serialize + DeserializeOwned + Send + Sync;
    fn payload(&self) -> &Self::Payload;
    fn key(&self) -> String;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Envelope<P> {
    pub event_id: Uuid,
    pub event_type: String,
    pub event_version: u32,
    pub timestamp: i64,
    pub source: String,
    pub trace_id: Option<String>,
    pub payload: P,
}

// --- Concrete events ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentCreated {
    pub document_id: DocumentId,
    pub content_hash: String,
    pub content_length: u64,
    pub metadata: serde_json::Value,
}

impl Event for DocumentCreated {
    const TOPIC: &'static str = "documents.created";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentUpdated {
    pub document_id: DocumentId,
    pub content_hash: String,
    pub previous_content_hash: String,
    pub content_length: u64,
    pub metadata: serde_json::Value,
}

impl Event for DocumentUpdated {
    const TOPIC: &'static str = "documents.updated";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentDeleted {
    pub document_id: DocumentId,
}

impl Event for DocumentDeleted {
    const TOPIC: &'static str = "documents.deleted";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingCreated {
    pub document_id: DocumentId,
    pub model: String,
    pub dim: u16,
    pub duration_ms: u64,
}

impl Event for EmbeddingCreated {
    const TOPIC: &'static str = "embeddings.created";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmbeddingFailed {
    pub document_id: DocumentId,
    pub model: String,
    pub attempts: u32,
    pub last_error: String,
    pub first_attempt_at: i64,
    pub last_attempt_at: i64,
}

impl Event for EmbeddingFailed {
    const TOPIC: &'static str = "embeddings.dlq";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiTelemetry {
    pub method: String,
    pub path: String,
    pub status: u16,
    pub duration_ms: u64,
    pub request_id: Uuid,
}

impl Event for ApiTelemetry {
    const TOPIC: &'static str = "telemetry.api";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.request_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyticsIngested {
    pub table: String,
    pub batch_size: u64,
    pub lag_ms: u64,
}

impl Event for AnalyticsIngested {
    const TOPIC: &'static str = "analytics.ingested";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.table.clone()
    }
}

// --- GraphRAG events ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntitiesExtracted {
    pub document_id: DocumentId,
    pub entity_count: u32,
    pub relationship_count: u32,
    pub model: String,
    pub duration_ms: u64,
}

impl Event for EntitiesExtracted {
    const TOPIC: &'static str = "entities.extracted";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractionFailed {
    pub document_id: DocumentId,
    pub model: String,
    pub attempts: u32,
    pub last_error: String,
    pub first_attempt_at: i64,
    pub last_attempt_at: i64,
}

impl Event for ExtractionFailed {
    const TOPIC: &'static str = "entities.dlq";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        self.document_id.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HnswRebuilt {
    pub vector_count: u64,
    pub build_duration_ms: u64,
    pub ef_construction: u32,
    pub m_param: u32,
}

impl Event for HnswRebuilt {
    const TOPIC: &'static str = "hnsw.rebuilt";
    const VERSION: u32 = 1;
    type Payload = Self;
    fn payload(&self) -> &Self {
        self
    }
    fn key(&self) -> String {
        format!("hnsw-{}", self.vector_count)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn roundtrip<E: Event>(event: &E, source: &str)
    where
        E::Payload: std::fmt::Debug + PartialEq + Clone,
    {
        let envelope = Envelope {
            event_id: Uuid::now_v7(),
            event_type: E::TOPIC.to_string(),
            event_version: E::VERSION,
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: source.to_string(),
            trace_id: Some("00-abcdef-01".into()),
            payload: event.payload().clone(),
        };
        let json = serde_json::to_string(&envelope).expect("serialize");
        let back: Envelope<E::Payload> = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(envelope.event_id, back.event_id);
        assert_eq!(envelope.event_type, back.event_type);
        assert_eq!(envelope.event_version, back.event_version);
        assert_eq!(envelope.timestamp, back.timestamp);
        assert_eq!(envelope.source, back.source);
        assert_eq!(envelope.trace_id, back.trace_id);
        assert_eq!(*event.payload(), back.payload);
    }

    #[test]
    fn document_created_roundtrip() {
        roundtrip(
            &DocumentCreated {
                document_id: DocumentId(Uuid::now_v7()),
                content_hash: "deadbeef".into(),
                content_length: 42,
                metadata: serde_json::json!({"key": "value"}),
            },
            "api-gateway",
        );
    }

    #[test]
    fn document_updated_roundtrip() {
        roundtrip(
            &DocumentUpdated {
                document_id: DocumentId(Uuid::now_v7()),
                content_hash: "aabbccdd".into(),
                previous_content_hash: "11223344".into(),
                content_length: 100,
                metadata: serde_json::json!({}),
            },
            "api-gateway",
        );
    }

    #[test]
    fn document_deleted_roundtrip() {
        roundtrip(
            &DocumentDeleted {
                document_id: DocumentId(Uuid::now_v7()),
            },
            "api-gateway",
        );
    }

    #[test]
    fn embedding_created_roundtrip() {
        roundtrip(
            &EmbeddingCreated {
                document_id: DocumentId(Uuid::now_v7()),
                model: "nomic-embed-text".into(),
                dim: 768,
                duration_ms: 42,
            },
            "embedding-worker",
        );
    }

    #[test]
    fn embedding_failed_roundtrip() {
        roundtrip(
            &EmbeddingFailed {
                document_id: DocumentId(Uuid::now_v7()),
                model: "nomic-embed-text".into(),
                attempts: 3,
                last_error: "timeout".into(),
                first_attempt_at: 1_746_943_200_000,
                last_attempt_at: 1_746_943_260_000,
            },
            "embedding-worker",
        );
    }

    #[test]
    fn api_telemetry_roundtrip() {
        roundtrip(
            &ApiTelemetry {
                method: "POST".into(),
                path: "/v1/documents".into(),
                status: 201,
                duration_ms: 12,
                request_id: Uuid::now_v7(),
            },
            "api-gateway",
        );
    }

    #[test]
    fn analytics_ingested_roundtrip() {
        roundtrip(
            &AnalyticsIngested {
                table: "documents_analytics".into(),
                batch_size: 1000,
                lag_ms: 340,
            },
            "analytics-consumer",
        );
    }

    #[test]
    fn entities_extracted_roundtrip() {
        roundtrip(
            &EntitiesExtracted {
                document_id: DocumentId(Uuid::now_v7()),
                entity_count: 5,
                relationship_count: 3,
                model: "llama3.2".into(),
                duration_ms: 1200,
            },
            "graph-worker",
        );
    }

    #[test]
    fn extraction_failed_roundtrip() {
        roundtrip(
            &ExtractionFailed {
                document_id: DocumentId(Uuid::now_v7()),
                model: "llama3.2".into(),
                attempts: 3,
                last_error: "parse error".into(),
                first_attempt_at: 1_746_943_200_000,
                last_attempt_at: 1_746_943_260_000,
            },
            "graph-worker",
        );
    }

    #[test]
    fn hnsw_rebuilt_roundtrip() {
        roundtrip(
            &HnswRebuilt {
                vector_count: 1_000_000,
                build_duration_ms: 45_000,
                ef_construction: 100,
                m_param: 16,
            },
            "api-gateway",
        );
    }
}
