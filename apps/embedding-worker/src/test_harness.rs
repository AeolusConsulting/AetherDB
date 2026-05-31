use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use aetherdb_domain::{ContentHash, Document, DocumentId};
use aetherdb_events::{EmbeddingCreated, EmbeddingFailed};
use aetherdb_storage::LibsqlStorage;
use tokio::sync::mpsc;

use crate::provider::EmbeddingProvider;
use crate::worker::Worker;

static HARNESS_DB_COUNTER: AtomicU64 = AtomicU64::new(0);

pub struct TestHarness {
    worker: Arc<Worker>,
    storage: Arc<LibsqlStorage>,
    #[allow(dead_code)]
    created_tx: mpsc::Sender<EmbeddingCreated>,
    created_rx: tokio::sync::Mutex<mpsc::Receiver<EmbeddingCreated>>,
    #[allow(dead_code)]
    dlq_tx: mpsc::Sender<EmbeddingFailed>,
    dlq_rx: tokio::sync::Mutex<mpsc::Receiver<EmbeddingFailed>>,
    batch_tx: mpsc::Sender<(DocumentId, String)>,
}

#[allow(clippy::expect_used)]
pub async fn spawn(provider: Arc<dyn EmbeddingProvider>) -> TestHarness {
    let db_id = HARNESS_DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_url = format!("file:worker_test_{db_id}?mode=memory&cache=shared");

    let storage = Arc::new(
        LibsqlStorage::connect(&db_url, None, None, 1)
            .await
            .expect("connect"),
    );
    storage.migrate().await.expect("migrate");

    let embed_dim = provider.dim();
    let worker = Arc::new(Worker::new(provider, storage.clone(), embed_dim));

    let (created_tx, created_rx) = mpsc::channel(256);
    let (dlq_tx, dlq_rx) = mpsc::channel(256);
    let (batch_tx, mut batch_rx) = mpsc::channel::<(DocumentId, String)>(256);

    let w = worker.clone();
    let ctx = created_tx.clone();
    let dtx = dlq_tx.clone();
    tokio::spawn(async move {
        let mut pending: Vec<(DocumentId, String)> = Vec::new();
        loop {
            let deadline = tokio::time::sleep(Duration::from_secs(1));
            tokio::pin!(deadline);

            tokio::select! {
                item = batch_rx.recv() => {
                    match item {
                        Some(pair) => {
                            pending.push(pair);
                            if pending.len() >= 32 {
                                let batch = std::mem::take(&mut pending);
                                process(&w, &batch, &ctx, &dtx).await;
                            }
                        }
                        None => {
                            if !pending.is_empty() {
                                let batch = std::mem::take(&mut pending);
                                process(&w, &batch, &ctx, &dtx).await;
                            }
                            break;
                        }
                    }
                }
                _ = &mut deadline => {
                    if !pending.is_empty() {
                        let batch = std::mem::take(&mut pending);
                        process(&w, &batch, &ctx, &dtx).await;
                    }
                }
            }
        }
    });

    TestHarness {
        worker,
        storage,
        created_tx,
        created_rx: tokio::sync::Mutex::new(created_rx),
        dlq_tx,
        dlq_rx: tokio::sync::Mutex::new(dlq_rx),
        batch_tx,
    }
}

async fn process(
    w: &Worker,
    batch: &[(DocumentId, String)],
    created_tx: &mpsc::Sender<EmbeddingCreated>,
    dlq_tx: &mpsc::Sender<EmbeddingFailed>,
) {
    let result = w.process_batch(batch).await;
    for ev in result.created {
        let _ = created_tx.send(ev).await;
    }
    for ev in result.dlq {
        let _ = dlq_tx.send(ev).await;
    }
}

impl TestHarness {
    pub async fn send_document_created(&self, doc_id: DocumentId, content: &str) {
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
        let _ = self.batch_tx.send((doc_id, content.to_string())).await;
    }

    pub async fn next_embedding_created(&self, timeout: Duration) -> Option<EmbeddingCreated> {
        let mut rx = self.created_rx.lock().await;
        tokio::time::timeout(timeout, rx.recv())
            .await
            .ok()
            .flatten()
    }

    pub async fn next_dlq(&self, timeout: Duration) -> Option<EmbeddingFailed> {
        let mut rx = self.dlq_rx.lock().await;
        tokio::time::timeout(timeout, rx.recv())
            .await
            .ok()
            .flatten()
    }

    pub async fn embedding_count_for(&self, doc_id: DocumentId) -> u64 {
        let model = self.worker.provider.model().clone();
        let exists = self
            .storage
            .embedding_repo()
            .exists(&doc_id, &model)
            .await
            .unwrap_or(false);
        if exists { 1 } else { 0 }
    }
}
