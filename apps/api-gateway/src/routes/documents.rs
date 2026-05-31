use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;

use aetherdb_domain::{
    CachedResponse, ContentHash, CreateDocumentRequest, CreateDocumentResponse, Document,
    DocumentDto, DocumentId, UpdateDocumentRequest,
};
use sha2::{Digest, Sha256};

use crate::state::AppState;

pub async fn create_document(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateDocumentRequest>,
) -> impl IntoResponse {
    metrics::counter!("http_requests_total", "method" => "POST", "path" => "/v1/documents", "status" => "201").increment(1);

    if req.content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "content must not be empty"})),
        )
            .into_response();
    }
    if req.content.len() > 1_048_576 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "content too large"})),
        )
            .into_response();
    }
    let metadata = if req.metadata.is_null() {
        serde_json::json!({})
    } else if req.metadata.is_object() {
        req.metadata
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "metadata must be an object or null"})),
        )
            .into_response();
    };

    let idem_key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if let Some(ref key) = idem_key {
        if let Ok(Some(cached)) = state.storage.idempotency_repo().get(key).await {
            let body: serde_json::Value = serde_json::from_str(&cached.body).unwrap_or_default();
            return (StatusCode::OK, Json(body)).into_response();
        }
    }

    let now = chrono::Utc::now();
    let id = DocumentId(uuid::Uuid::now_v7());
    let mut hasher = Sha256::new();
    hasher.update(req.content.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();

    let doc = Document {
        id,
        content: req.content,
        metadata,
        content_hash: ContentHash(hash),
        created_at: now,
        updated_at: now,
    };

    if let Err(e) = state.storage.document_repo().insert(&doc).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    let resp = CreateDocumentResponse {
        id: doc.id,
        created_at: doc.created_at,
    };
    let resp_body = serde_json::to_string(&resp).unwrap_or_default();

    if let Some(ref key) = idem_key {
        let cached = CachedResponse {
            status_code: 201,
            body: resp_body.clone(),
        };
        let _ = state
            .storage
            .idempotency_repo()
            .put(key, &cached, Duration::from_secs(86_400))
            .await;
    }

    if let Some(ref producer) = state.producer {
        let event = aetherdb_events::DocumentCreated {
            document_id: doc.id,
            content_hash: hex::encode(hash),
            content_length: doc.content.len() as u64,
            metadata: doc.metadata.clone(),
        };
        if let Err(e) = producer.publish(&event).await {
            tracing::warn!("failed to publish documents.created event: {e}");
        }
    }

    let body: serde_json::Value = serde_json::from_str(&resp_body).unwrap_or_default();
    (StatusCode::CREATED, Json(body)).into_response()
}

pub async fn get_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid id"})),
            )
                .into_response();
        }
    };

    match state.storage.document_repo().get(&DocumentId(uid)).await {
        Ok(Some(doc)) => {
            let dto = DocumentDto {
                id: doc.id,
                content: doc.content,
                metadata: doc.metadata,
                created_at: doc.created_at,
                updated_at: doc.updated_at,
            };
            Json(serde_json::to_value(dto).unwrap_or_default()).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn delete_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let uid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    match state.storage.document_repo().delete(&DocumentId(uid)).await {
        Ok(true) => {
            if let Some(ref producer) = state.producer {
                let event = aetherdb_events::DocumentDeleted {
                    document_id: DocumentId(uid),
                };
                if let Err(e) = producer.publish(&event).await {
                    tracing::warn!("failed to publish documents.deleted event: {e}");
                }
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

pub async fn update_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateDocumentRequest>,
) -> impl IntoResponse {
    let uid = match uuid::Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid id"})),
            )
                .into_response();
        }
    };

    if req.content.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "content must not be empty"})),
        )
            .into_response();
    }
    if req.content.len() > 1_048_576 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "content too large"})),
        )
            .into_response();
    }

    let metadata = if req.metadata.is_null() {
        serde_json::json!({})
    } else if req.metadata.is_object() {
        req.metadata
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "metadata must be an object or null"})),
        )
            .into_response();
    };

    let mut hasher = Sha256::new();
    hasher.update(req.content.as_bytes());
    let hash: [u8; 32] = hasher.finalize().into();

    let doc = Document {
        id: DocumentId(uid),
        content: req.content,
        metadata,
        content_hash: ContentHash(hash),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    match state.storage.document_repo().update(&doc).await {
        Ok(true) => {
            if let Some(ref producer) = state.producer {
                let event = aetherdb_events::DocumentUpdated {
                    document_id: doc.id,
                    content_hash: hex::encode(hash),
                    previous_content_hash: String::new(),
                    content_length: doc.content.len() as u64,
                    metadata: doc.metadata.clone(),
                };
                if let Err(e) = producer.publish(&event).await {
                    tracing::warn!("failed to publish documents.updated event: {e}");
                }
            }
            Json(serde_json::json!({"id": doc.id, "updated_at": doc.updated_at})).into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}
