// Contract C6 — embedding worker
//
// These tests use a mock embedding provider (no Ollama/OpenAI dependency)
// and an in-process messaging mock to verify worker behaviour.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use aetherdb_domain::{DocumentId, EmbeddingModel};
use aetherdb_embedding_worker::{Worker, EmbeddingProvider, ProviderError};

// ---- mock providers ----

struct AlwaysOkProvider { dim: u16 }
#[async_trait::async_trait]
impl EmbeddingProvider for AlwaysOkProvider {
    fn model(&self) -> &EmbeddingModel { static M: once_cell::sync::Lazy<EmbeddingModel> =
        once_cell::sync::Lazy::new(|| EmbeddingModel("mock-ok".into())); &M }
    fn dim(&self) -> u16 { self.dim }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        Ok(texts.iter().map(|_| vec![0.1; self.dim as usize]).collect())
    }
}

struct Fails500NTimes { remaining: Arc<AtomicU32>, dim: u16 }
#[async_trait::async_trait]
impl EmbeddingProvider for Fails500NTimes {
    fn model(&self) -> &EmbeddingModel { static M: once_cell::sync::Lazy<EmbeddingModel> =
        once_cell::sync::Lazy::new(|| EmbeddingModel("mock-500".into())); &M }
    fn dim(&self) -> u16 { self.dim }
    async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        if self.remaining.fetch_sub(1, Ordering::SeqCst) > 0 {
            Err(ProviderError::Retryable("HTTP 500".into()))
        } else {
            Ok(vec![vec![0.1; self.dim as usize]])
        }
    }
}

struct WrongDimProvider { real_dim: u16, lies_about: u16 }
#[async_trait::async_trait]
impl EmbeddingProvider for WrongDimProvider {
    fn model(&self) -> &EmbeddingModel { static M: once_cell::sync::Lazy<EmbeddingModel> =
        once_cell::sync::Lazy::new(|| EmbeddingModel("mock-wrong-dim".into())); &M }
    fn dim(&self) -> u16 { self.lies_about }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        // Returns wrong-dim vectors.
        Ok(texts.iter().map(|_| vec![0.1; self.real_dim as usize]).collect())
    }
}

// ---- tests ----

#[tokio::test]
async fn happy_path_embeds_and_publishes() {
    let h = aetherdb_embedding_worker::test_harness::spawn(Arc::new(AlwaysOkProvider { dim: 768 })).await;
    h.send_document_created(DocumentId(uuid::Uuid::now_v7()), "hello").await;
    let event = h.next_embedding_created(Duration::from_secs(5)).await.expect("event arrived");
    assert_eq!(event.dim, 768);
}

#[tokio::test]
async fn provider_500_three_times_then_dlq() {
    let remaining = Arc::new(AtomicU32::new(10)); // never succeeds
    let provider = Arc::new(Fails500NTimes { remaining, dim: 768 });
    let h = aetherdb_embedding_worker::test_harness::spawn(provider).await;
    let doc_id = DocumentId(uuid::Uuid::now_v7());
    h.send_document_created(doc_id, "x").await;
    let dlq = h.next_dlq(Duration::from_secs(30)).await.expect("dlq event arrived");
    assert_eq!(dlq.document_id, doc_id);
    assert_eq!(dlq.attempts, 3);
}

#[tokio::test]
async fn wrong_dim_response_goes_to_dlq_immediately() {
    let provider = Arc::new(WrongDimProvider { real_dim: 512, lies_about: 768 });
    let h = aetherdb_embedding_worker::test_harness::spawn(provider).await;
    let doc_id = DocumentId(uuid::Uuid::now_v7());
    h.send_document_created(doc_id, "x").await;
    let dlq = h.next_dlq(Duration::from_secs(5)).await.expect("dlq event arrived");
    assert_eq!(dlq.document_id, doc_id);
    assert!(dlq.last_error.contains("DimensionMismatch"),
        "dlq last_error must mention DimensionMismatch, got: {}", dlq.last_error);
    assert_eq!(dlq.attempts, 1, "dim mismatch is non-retryable, attempts must be 1");
}

#[tokio::test]
async fn duplicate_document_created_is_noop() {
    let h = aetherdb_embedding_worker::test_harness::spawn(Arc::new(AlwaysOkProvider { dim: 768 })).await;
    let doc_id = DocumentId(uuid::Uuid::now_v7());
    h.send_document_created(doc_id, "first").await;
    let _ = h.next_embedding_created(Duration::from_secs(5)).await.expect("first event");
    // Send the same document_id again — should NOT produce a second embeddings.created.
    h.send_document_created(doc_id, "first").await;
    let second = h.next_embedding_created(Duration::from_secs(2)).await;
    assert!(second.is_none(), "duplicate should not produce a second event");
    // Storage should still have exactly one embedding row.
    assert_eq!(h.embedding_count_for(doc_id).await, 1);
}

#[tokio::test]
async fn batching_groups_documents_arriving_quickly() {
    let calls = Arc::new(AtomicU32::new(0));
    let provider = Arc::new(CountingProvider { calls: calls.clone(), dim: 768 });
    let h = aetherdb_embedding_worker::test_harness::spawn(provider).await;
    for _ in 0..50 {
        h.send_document_created(DocumentId(uuid::Uuid::now_v7()), "x").await;
    }
    // Wait long enough for all embeddings to be processed.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let total_calls = calls.load(Ordering::SeqCst);
    assert!(total_calls <= 2,
        "50 documents arriving simultaneously should batch into ≤ 2 provider calls, got {total_calls}");
}

struct CountingProvider { calls: Arc<AtomicU32>, dim: u16 }
#[async_trait::async_trait]
impl EmbeddingProvider for CountingProvider {
    fn model(&self) -> &EmbeddingModel { static M: once_cell::sync::Lazy<EmbeddingModel> =
        once_cell::sync::Lazy::new(|| EmbeddingModel("mock-count".into())); &M }
    fn dim(&self) -> u16 { self.dim }
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, ProviderError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(texts.iter().map(|_| vec![0.1; self.dim as usize]).collect())
    }
}
