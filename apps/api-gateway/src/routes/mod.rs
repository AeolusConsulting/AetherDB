pub mod documents;
pub mod graph;
pub mod health;
pub mod metrics;
pub mod search;
pub mod sync_api;
pub mod warehouse;

use axum::Router;
use axum::routing::{get, post};

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/readiness", get(health::readiness))
        .route("/metrics", get(metrics::metrics))
        .route("/v1/documents", post(documents::create_document))
        .route("/v1/documents/list", get(warehouse::list_documents))
        .route("/v1/documents/bulk", post(warehouse::bulk_import))
        .route(
            "/v1/documents/:id",
            get(documents::get_document)
                .put(documents::update_document)
                .delete(documents::delete_document),
        )
        .route("/v1/search/semantic", post(search::semantic_search))
        .route("/v1/search/hybrid", post(search::hybrid_search))
        .route("/v1/query", post(warehouse::sql_query))
        .route("/v1/graph/entities", get(graph::get_entities_by_document))
        .route(
            "/v1/graph/entities/:name/related",
            get(graph::get_related_documents),
        )
        .route("/v1/sync/pull", post(sync_api::sync_pull))
        .route("/v1/sync/push", post(sync_api::sync_push))
        .with_state(state)
}
