// Contract C3 — storage layer
//
// All tests run against in-memory libSQL. They MUST fail until crates/storage
// exposes the contract API with working impls.

use aetherdb_domain::{Document, DocumentId, ContentHash, Embedding, EmbeddingModel};
use aetherdb_storage::LibsqlStorage;
use chrono::Utc;
use std::sync::Arc;
use std::time::Duration;

async fn fresh_storage() -> LibsqlStorage {
    let s = LibsqlStorage::connect("file::memory:?cache=shared", None, None, 1).await.expect("connect");
    s.migrate().await.expect("migrate");
    s
}

fn make_doc(content: &str) -> Document {
    Document {
        id: DocumentId(uuid::Uuid::now_v7()),
        content: content.to_string(),
        metadata: serde_json::json!({}),
        content_hash: ContentHash([0; 32]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[tokio::test]
async fn insert_and_get_roundtrip() {
    let s = fresh_storage().await;
    let repo = s.document_repo();
    let doc = make_doc("hello world");
    repo.insert(&doc).await.expect("insert");
    let got = repo.get(&doc.id).await.expect("get").expect("found");
    assert_eq!(got.content, "hello world");
    assert_eq!(got.id, doc.id);
}

#[tokio::test]
async fn get_missing_returns_none() {
    let s = fresh_storage().await;
    let repo = s.document_repo();
    let got = repo.get(&DocumentId(uuid::Uuid::now_v7())).await.expect("get");
    assert!(got.is_none());
}

#[tokio::test]
async fn delete_returns_true_when_existed_false_when_not() {
    let s = fresh_storage().await;
    let repo = s.document_repo();
    let doc = make_doc("x");
    repo.insert(&doc).await.expect("insert");
    assert!(repo.delete(&doc.id).await.expect("delete"));
    assert!(!repo.delete(&doc.id).await.expect("delete again"));
}

#[tokio::test]
async fn migrations_are_idempotent() {
    let s = LibsqlStorage::connect("file::memory:?cache=shared", None, None, 1).await.expect("connect");
    s.migrate().await.expect("migrate 1");
    s.migrate().await.expect("migrate 2 (must be no-op)");
}

#[tokio::test]
async fn concurrent_inserts_all_succeed_with_unique_ids() {
    let s = Arc::new(fresh_storage().await);
    let mut handles = Vec::new();
    for i in 0..100 {
        let s = s.clone();
        handles.push(tokio::spawn(async move {
            let doc = make_doc(&format!("doc-{i}"));
            s.document_repo().insert(&doc).await.expect("insert");
            doc.id
        }));
    }
    let mut ids = Vec::new();
    for h in handles { ids.push(h.await.unwrap()); }
    ids.sort_by_key(|d| d.0);
    let unique = ids.windows(2).all(|w| w[0] != w[1]);
    assert!(unique, "concurrent inserts produced duplicate IDs");
    assert_eq!(ids.len(), 100);
}

#[tokio::test]
async fn delete_cascades_to_embedding() {
    let s = fresh_storage().await;
    let doc = make_doc("x");
    s.document_repo().insert(&doc).await.expect("insert doc");
    let emb = Embedding {
        document_id: doc.id,
        vector: vec![0.1; 768],
        model: EmbeddingModel("nomic-embed-text".into()),
        dim: 768,
        created_at: Utc::now(),
    };
    s.embedding_repo().upsert(&emb).await.expect("upsert embedding");
    assert!(s.embedding_repo().exists(&doc.id, &emb.model).await.expect("exists"));
    s.document_repo().delete(&doc.id).await.expect("delete doc");
    assert!(!s.embedding_repo().exists(&doc.id, &emb.model).await.expect("exists after delete"),
        "embedding row was not cascade-deleted");
}

#[tokio::test]
async fn transaction_rollback_leaves_no_trace() {
    let s = fresh_storage().await;
    let doc = make_doc("rolled-back");
    let doc_id = doc.id;
    let result: Result<(), _> = s.with_transaction(|tx| Box::pin(async move {
        tx.insert_document(&doc).await?;
        Err::<(), _>(aetherdb_domain::StorageError::Other("intentional".into()))
    })).await;
    assert!(result.is_err());
    assert!(s.document_repo().get(&doc_id).await.expect("get").is_none(),
        "transaction was not rolled back");
}

#[tokio::test]
async fn count_recent_without_embedding_counts_correctly() {
    let s = fresh_storage().await;
    let d1 = make_doc("one");
    let d2 = make_doc("two");
    s.document_repo().insert(&d1).await.expect("insert 1");
    s.document_repo().insert(&d2).await.expect("insert 2");
    s.embedding_repo().upsert(&Embedding {
        document_id: d1.id,
        vector: vec![0.1; 768],
        model: EmbeddingModel("nomic-embed-text".into()),
        dim: 768,
        created_at: Utc::now(),
    }).await.expect("upsert");
    let n = s.document_repo()
        .count_recent_without_embedding(Duration::from_secs(60))
        .await.expect("count");
    assert_eq!(n, 1, "only d2 should lack an embedding");
}

#[tokio::test]
async fn idempotency_put_and_get() {
    let s = fresh_storage().await;
    let repo = s.idempotency_repo();
    let key = "test-key-1";
    assert!(repo.get(key).await.expect("get").is_none());
    let resp = aetherdb_domain::CachedResponse {
        status_code: 201,
        body: r#"{"id":"abc"}"#.to_string(),
    };
    repo.put(key, &resp, Duration::from_secs(86_400)).await.expect("put");
    let got = repo.get(key).await.expect("get").expect("present");
    assert_eq!(got.status_code, 201);
    assert_eq!(got.body, resp.body);
}

#[tokio::test]
async fn idempotency_sweep_removes_expired() {
    let s = fresh_storage().await;
    let repo = s.idempotency_repo();
    let resp = aetherdb_domain::CachedResponse { status_code: 200, body: "{}".into() };
    repo.put("k1", &resp, Duration::from_millis(50)).await.expect("put short ttl");
    tokio::time::sleep(Duration::from_millis(100)).await;
    let swept = repo.sweep_expired().await.expect("sweep");
    assert!(swept >= 1, "sweep should have removed at least one expired key");
}
