use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use aetherdb_domain::{
    DocumentId, EntitiesResponse, EntityDto, RelatedDocumentHit, RelatedDocumentsResponse,
};

use crate::state::AppState;

#[derive(serde::Deserialize)]
pub struct DocumentIdQuery {
    pub document_id: String,
}

#[derive(serde::Deserialize)]
pub struct DepthQuery {
    #[serde(default = "default_depth")]
    pub depth: u32,
}

fn default_depth() -> u32 {
    2
}

pub async fn get_entities_by_document(
    State(state): State<AppState>,
    Query(query): Query<DocumentIdQuery>,
) -> impl IntoResponse {
    let uid = match uuid::Uuid::parse_str(&query.document_id) {
        Ok(u) => u,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid document_id"})),
            )
                .into_response();
        }
    };

    let entities = match state
        .storage
        .entity_repo()
        .get_by_document(&DocumentId(uid))
        .await
    {
        Ok(e) => e,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let dtos: Vec<EntityDto> = entities
        .into_iter()
        .map(|e| EntityDto {
            id: e.id,
            name: e.name,
            entity_type: e.entity_type,
            document_id: e.document_id,
            created_at: e.created_at,
        })
        .collect();

    Json(EntitiesResponse { entities: dtos }).into_response()
}

pub async fn get_related_documents(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Query(query): Query<DepthQuery>,
) -> impl IntoResponse {
    let querier = match &state.graph_querier {
        Some(q) => q,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error": "graph not enabled"})),
            )
                .into_response();
        }
    };

    if query.depth > 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "depth must be <= 5"})),
        )
            .into_response();
    }

    let results = match querier.find_related_documents(&name, query.depth).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let documents: Vec<RelatedDocumentHit> = results
        .into_iter()
        .map(|(doc_id, hop)| RelatedDocumentHit {
            document_id: doc_id,
            entity_name: name.clone(),
            hop_count: hop,
        })
        .collect();

    Json(RelatedDocumentsResponse { documents }).into_response()
}
