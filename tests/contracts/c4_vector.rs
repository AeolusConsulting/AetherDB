// Contract C4 — vector search
//
// These tests assert the SQL query shape, the score conversion, the validation,
// and the EXPLAIN QUERY PLAN. They must fail until crates/vector is implemented.

use aetherdb_domain::{Document, DocumentId, ContentHash, Embedding, EmbeddingModel};
use aetherdb_storage::LibsqlStorage;
use aetherdb_vector::{VectorSearcher, VectorError};
use chrono::Utc;
use std::sync::Arc;

const DIM: u16 = 768;

async fn fresh_storage_with_embedding(content: &str, embedding: Vec<f32>) -> (Arc<LibsqlStorage>, DocumentId) {
    let s = Arc::new(LibsqlStorage::connect("file::memory:?cache=shared", None, None, 1).await.expect("connect"));
    s.migrate().await.expect("migrate");
    let doc_id = DocumentId(uuid::Uuid::now_v7());
    s.document_repo().insert(&Document {
        id: doc_id,
        content: content.into(),
        metadata: serde_json::json!({}),
        content_hash: ContentHash([0; 32]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }).await.expect("insert doc");
    s.embedding_repo().upsert(&Embedding {
        document_id: doc_id,
        vector: embedding,
        model: EmbeddingModel("nomic-embed-text".into()),
        dim: DIM,
        created_at: Utc::now(),
    }).await.expect("upsert embedding");
    (s, doc_id)
}

fn random_vec(dim: usize, seed: u64) -> Vec<f32> {
    // Deterministic pseudo-random — same seed produces same vector.
    let mut x: u64 = seed.wrapping_mul(0x9E3779B97F4A7C15);
    (0..dim).map(|_| {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
        ((x as u32) as f32 / u32::MAX as f32) * 2.0 - 1.0
    }).collect()
}

#[tokio::test]
async fn dimension_mismatch_returns_typed_error() {
    let (s, _) = fresh_storage_with_embedding("x", random_vec(DIM as usize, 1)).await;
    let v = VectorSearcher::new(s, DIM);
    let bad_query = vec![0.5_f32; 100]; // wrong dim
    match v.search_similar(&bad_query, 10, None).await {
        Err(VectorError::DimensionMismatch { expected, got }) => {
            assert_eq!(expected, DIM as usize);
            assert_eq!(got, 100);
        }
        other => panic!("expected DimensionMismatch, got {:?}", other),
    }
}

#[tokio::test]
async fn nan_in_query_vector_returns_non_finite() {
    let (s, _) = fresh_storage_with_embedding("x", random_vec(DIM as usize, 1)).await;
    let v = VectorSearcher::new(s, DIM);
    let mut q = vec![0.5_f32; DIM as usize];
    q[0] = f32::NAN;
    assert!(matches!(v.search_similar(&q, 10, None).await, Err(VectorError::NonFinite)));
}

#[tokio::test]
async fn inf_in_query_vector_returns_non_finite() {
    let (s, _) = fresh_storage_with_embedding("x", random_vec(DIM as usize, 1)).await;
    let v = VectorSearcher::new(s, DIM);
    let mut q = vec![0.5_f32; DIM as usize];
    q[1] = f32::INFINITY;
    assert!(matches!(v.search_similar(&q, 10, None).await, Err(VectorError::NonFinite)));
}

#[tokio::test]
async fn invalid_top_k_returns_typed_error() {
    let (s, _) = fresh_storage_with_embedding("x", random_vec(DIM as usize, 1)).await;
    let v = VectorSearcher::new(s, DIM);
    let q = vec![0.5_f32; DIM as usize];
    assert!(matches!(v.search_similar(&q, 0, None).await, Err(VectorError::InvalidTopK(_))));
    assert!(matches!(v.search_similar(&q, 101, None).await, Err(VectorError::InvalidTopK(_))));
}

#[tokio::test]
async fn identical_vector_yields_score_above_0_99() {
    let v_seed = random_vec(DIM as usize, 42);
    let (s, doc_id) = fresh_storage_with_embedding("known", v_seed.clone()).await;
    let searcher = VectorSearcher::new(s, DIM);
    let results = searcher.search_similar(&v_seed, 5, None).await.expect("search");
    assert!(!results.is_empty(), "must return at least one result");
    let top = &results[0];
    assert_eq!(top.document_id, doc_id);
    assert!(top.score >= 0.99,
        "identical vector should score >= 0.99, got {}", top.score);
}

#[tokio::test]
async fn min_score_filter_excludes_low_similarity() {
    // Seed two documents with orthogonal-ish vectors. Query with one of them.
    // The other should be excluded by a high min_score.
    let s = Arc::new(LibsqlStorage::connect("file::memory:?cache=shared", None, None, 1).await.expect("connect"));
    s.migrate().await.expect("migrate");
    let v_a: Vec<f32> = (0..DIM).map(|i| if i == 0 { 1.0 } else { 0.0 }).collect();
    let v_b: Vec<f32> = (0..DIM).map(|i| if i == 1 { 1.0 } else { 0.0 }).collect();
    for (content, v) in [("a", &v_a), ("b", &v_b)] {
        let doc_id = DocumentId(uuid::Uuid::now_v7());
        s.document_repo().insert(&Document {
            id: doc_id, content: content.into(), metadata: serde_json::json!({}),
            content_hash: ContentHash([0;32]), created_at: Utc::now(), updated_at: Utc::now(),
        }).await.unwrap();
        s.embedding_repo().upsert(&Embedding {
            document_id: doc_id, vector: v.clone(), model: EmbeddingModel("m".into()),
            dim: DIM, created_at: Utc::now(),
        }).await.unwrap();
    }
    let searcher = VectorSearcher::new(s, DIM);
    let results = searcher.search_similar(&v_a, 5, Some(0.9)).await.expect("search");
    // v_a vs v_b are orthogonal → cosine similarity ≈ 0 → must be filtered out.
    for r in &results {
        assert!(r.score >= 0.9, "result with score {} slipped past min_score 0.9", r.score);
    }
}

#[tokio::test]
async fn explain_query_plan_uses_the_vector_index() {
    // This is the key regression test: the production query MUST hit the
    // index. We achieve this by inspecting EXPLAIN QUERY PLAN output for
    // the canonical SQL. The test calls into a debug helper that returns
    // the plan string.
    let (s, _) = fresh_storage_with_embedding("x", random_vec(DIM as usize, 7)).await;
    let plan = aetherdb_vector::debug_explain_search_plan(&s, DIM).await.expect("explain");
    eprintln!("plan: {plan}");
    assert!(plan.contains("VIRTUAL TABLE INDEX"),
        "EXPLAIN QUERY PLAN must show vector index usage; got:\n{plan}");
    assert!(!plan.contains("SCAN document_embeddings"),
        "EXPLAIN QUERY PLAN must NOT show a full scan of document_embeddings; got:\n{plan}");
}

#[tokio::test]
async fn l2_normalize_unit_vector_is_idempotent() {
    let mut v = vec![3.0_f32, 4.0]; // length 5
    aetherdb_vector::l2_normalize(&mut v);
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    assert!((len - 1.0).abs() < 1e-6, "post-normalize length is {len}, want 1.0");
}

#[tokio::test]
async fn l2_normalize_zero_vector_unchanged() {
    let mut v = vec![0.0_f32; 4];
    aetherdb_vector::l2_normalize(&mut v);
    assert!(v.iter().all(|&x| x == 0.0), "zero vector must be left alone");
}
