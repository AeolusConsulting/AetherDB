use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use aetherdb_domain::{ContentHash, Document, DocumentId};
use aetherdb_events::{EntitiesExtracted, ExtractionFailed};
use aetherdb_storage::LibsqlStorage;
use tokio::sync::mpsc;

use crate::provider::LlmProvider;
use crate::worker::Worker;

static HARNESS_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TestHarness {
    worker: Arc<Worker>,
    storage: Arc<LibsqlStorage>,
    created_rx: tokio::sync::Mutex<mpsc::Receiver<EntitiesExtracted>>,
    dlq_rx: tokio::sync::Mutex<mpsc::Receiver<ExtractionFailed>>,
    created_tx: mpsc::Sender<EntitiesExtracted>,
    dlq_tx: mpsc::Sender<ExtractionFailed>,
}

#[allow(clippy::expect_used)]
pub async fn spawn(provider: Arc<dyn LlmProvider>) -> TestHarness {
    let db_id = HARNESS_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_url = format!("file:graph_test_{db_id}?mode=memory&cache=shared");

    let storage = Arc::new(
        LibsqlStorage::connect(&db_url, None, None, 1)
            .await
            .expect("connect"),
    );
    storage.migrate().await.expect("migrate");

    let worker = Arc::new(Worker::new(provider, storage.clone()));
    let (created_tx, created_rx) = mpsc::channel(256);
    let (dlq_tx, dlq_rx) = mpsc::channel(256);

    TestHarness {
        worker,
        storage,
        created_rx: tokio::sync::Mutex::new(created_rx),
        dlq_rx: tokio::sync::Mutex::new(dlq_rx),
        created_tx,
        dlq_tx,
    }
}

impl TestHarness {
    pub async fn send_document(&self, doc_id: DocumentId, content: &str) {
        let now = chrono::Utc::now();
        let doc = Document {
            id: doc_id,
            content: content.to_string(),
            metadata: serde_json::json!({}),
            content_hash: ContentHash([0; 32]),
            created_at: now,
            updated_at: now,
        };
        let _ = self.storage.document_repo().insert(&doc).await;

        let result = self.worker.process_document(doc_id, content).await;
        for ev in result.created {
            let _ = self.created_tx.send(ev).await;
        }
        for ev in result.dlq {
            let _ = self.dlq_tx.send(ev).await;
        }
    }

    pub async fn next_extracted(&self, timeout: Duration) -> Option<EntitiesExtracted> {
        let mut rx = self.created_rx.lock().await;
        tokio::time::timeout(timeout, rx.recv())
            .await
            .ok()
            .flatten()
    }

    pub async fn next_dlq(&self, timeout: Duration) -> Option<ExtractionFailed> {
        let mut rx = self.dlq_rx.lock().await;
        tokio::time::timeout(timeout, rx.recv())
            .await
            .ok()
            .flatten()
    }

    pub async fn entity_count_for(&self, doc_id: DocumentId) -> u64 {
        self.storage
            .entity_repo()
            .get_by_document(&doc_id)
            .await
            .map(|v| v.len() as u64)
            .unwrap_or(0)
    }

    pub fn storage(&self) -> &Arc<LibsqlStorage> {
        &self.storage
    }
}
