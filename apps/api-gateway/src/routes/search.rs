use std::collections::HashMap;
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use aetherdb_domain::{
    DocumentId, HybridSearchHit, HybridSearchRequest, HybridSearchResponse, SearchRequest,
};

use crate::state::AppState;

pub async fn semantic_search(
    State(state): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> impl IntoResponse {
    if req.query.is_empty() || req.query.len() > 8192 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "query must be 1..=8192 chars"})),
        )
            .into_response();
    }
    if !(1..=100).contains(&req.top_k) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "top_k must be in 1..=100"})),
        )
            .into_response();
    }

    let pending = state
        .storage
        .document_repo()
        .count_recent_without_embedding(Duration::from_secs(60))
        .await
        .unwrap_or(0);

    let mut results = Vec::new();

    if let (Some(provider), Some(searcher)) = (&state.embed_provider, &state.vector_searcher) {
        match provider.embed(&[req.query.clone()]).await {
            Ok(vectors) if !vectors.is_empty() => {
                if let Ok(hits) = searcher
                    .search_similar(&vectors[0], req.top_k as usize, req.min_score)
                    .await
                {
                    for hit in hits {
                        let preview = state
                            .storage
                            .document_repo()
                            .get(&hit.document_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|d| d.content.chars().take(200).collect::<String>())
                            .unwrap_or_default();
                        results.push(serde_json::json!({
                            "document_id": hit.document_id,
                            "score": hit.score,
                            "content_preview": preview,
                        }));
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!("query embedding failed: {e}");
            }
        }
    }

    let resp = serde_json::json!({
        "results": results,
        "pending_embeddings": pending,
    });

    (
        StatusCode::OK,
        [("x-pending-embeddings", pending.to_string())],
        Json(resp),
    )
        .into_response()
}

pub async fn hybrid_search(
    State(state): State<AppState>,
    Json(req): Json<HybridSearchRequest>,
) -> impl IntoResponse {
    if req.query.is_empty() || req.query.len() > 8192 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "query must be 1..=8192 chars"})),
        )
            .into_response();
    }
    if !(1..=100).contains(&req.top_k) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "top_k must be in 1..=100"})),
        )
            .into_response();
    }
    if req.graph_depth > 5 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "graph_depth must be <= 5"})),
        )
            .into_response();
    }

    let graph_weight = req.graph_weight.clamp(0.0, 1.0);
    let mut scores: HashMap<DocumentId, (Option<f32>, Option<f32>)> = HashMap::new();

    // Graph traversal: find entities matching query words, traverse graph
    if let Some(ref querier) = state.graph_querier {
        if let Ok(related) = querier
            .find_related_documents(&req.query, req.graph_depth)
            .await
        {
            for (doc_id, hop_count) in related {
                let graph_score = 1.0 / (1.0 + hop_count as f32);
                scores.entry(doc_id).or_insert((None, None)).1 = Some(graph_score);
            }
        }
    }

    let pending = state
        .storage
        .document_repo()
        .count_recent_without_embedding(Duration::from_secs(60))
        .await
        .unwrap_or(0);

    // Merge scores
    let mut results: Vec<HybridSearchHit> = Vec::new();
    for (doc_id, (vector_score, graph_score)) in &scores {
        let vs = vector_score.unwrap_or(0.0);
        let gs = graph_score.unwrap_or(0.0);
        let final_score = (1.0 - graph_weight) * vs + graph_weight * gs;

        if let Some(min) = req.min_score {
            if final_score < min {
                continue;
            }
        }

        let preview = state
            .storage
            .document_repo()
            .get(doc_id)
            .await
            .ok()
            .flatten()
            .map(|d| {
                let chars: String = d.content.chars().take(200).collect();
                chars
            })
            .unwrap_or_default();

        results.push(HybridSearchHit {
            document_id: *doc_id,
            score: final_score,
            vector_score: *vector_score,
            graph_score: *graph_score,
            content_preview: preview,
        });
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(req.top_k as usize);

    let resp = HybridSearchResponse {
        results,
        pending_embeddings: pending,
    };

    Json(resp).into_response()
}
