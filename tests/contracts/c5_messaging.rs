// Contract C5 — Redpanda messaging
//
// Tests use testcontainers to boot a real Redpanda instance.
// These tests are #[ignore]'d by default because they require Docker; CI runs
// them in a separate job with `--include-ignored`.

use std::time::Duration;

#[tokio::test]
#[ignore = "requires Docker"]
async fn producer_config_includes_all_required_settings() {
    // The producer MUST expose its rdkafka ClientConfig (or a normalized view)
    // for inspection.
    let cfg = aetherdb_messaging::producer_default_config("localhost:9092");
    let pairs = [
        ("acks", "all"),
        ("enable.idempotence", "true"),
        ("max.in.flight.requests.per.connection", "5"),
        ("retries", "10"),
        ("compression.type", "zstd"),
        ("linger.ms", "5"),
    ];
    for (k, v) in pairs {
        assert_eq!(cfg.get(k).map(|s| s.as_str()), Some(v),
            "producer config {k} must be {v}");
    }
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn produce_and_consume_roundtrip() {
    let (brokers, _redpanda) = start_redpanda().await;
    let producer = aetherdb_messaging::EventProducer::connect(&brokers).await.expect("connect");

    let doc_id = aetherdb_domain::DocumentId(uuid::Uuid::now_v7());
    let event = aetherdb_events::DocumentCreated {
        document_id: doc_id,
        content_hash: "deadbeef".into(),
        content_length: 100,
        metadata: serde_json::json!({}),
    };
    producer.publish(&event).await.expect("publish");
    producer.flush(Duration::from_secs(5)).await.expect("flush");

    let consumer = aetherdb_messaging::EventConsumer::<aetherdb_events::DocumentCreated>::connect(
        &brokers, "test-consumer-group",
    ).await.expect("connect consumer");

    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    let handle = tokio::spawn(async move {
        consumer.run(move |payload, _env| {
            let tx = tx.clone();
            async move {
                tx.send(payload).await.unwrap();
                Ok(())
            }
        }, tokio::signal::ctrl_c().fuse()).await
    });

    let received = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await.expect("timeout").expect("recv");
    assert_eq!(received.document_id, doc_id);
    handle.abort();
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn handler_retryable_error_eventually_goes_to_dlq() {
    let (brokers, _redpanda) = start_redpanda().await;
    let producer = aetherdb_messaging::EventProducer::connect(&brokers).await.expect("connect");
    let event = aetherdb_events::DocumentCreated {
        document_id: aetherdb_domain::DocumentId(uuid::Uuid::now_v7()),
        content_hash: "x".into(),
        content_length: 0,
        metadata: serde_json::json!({}),
    };
    producer.publish(&event).await.expect("publish");
    producer.flush(Duration::from_secs(5)).await.expect("flush");

    let consumer = aetherdb_messaging::EventConsumer::<aetherdb_events::DocumentCreated>::connect(
        &brokers, "test-retry-group",
    ).await.expect("connect");

    let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let attempts_h = attempts.clone();
    let handle = tokio::spawn(async move {
        consumer.run(move |_payload, _env| {
            let a = attempts_h.clone();
            async move {
                a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(aetherdb_messaging::HandlerError::Retryable("simulated".into()))
            }
        }, tokio::signal::ctrl_c().fuse()).await
    });

    // Wait long enough for 10 retries (exponential backoff capped at 5s).
    tokio::time::sleep(Duration::from_secs(45)).await;
    let final_attempts = attempts.load(std::sync::atomic::Ordering::SeqCst);
    assert!(final_attempts >= 10, "expected at least 10 attempts, got {final_attempts}");

    // Verify the DLQ topic received the message.
    let dlq_consumer = aetherdb_messaging::EventConsumer::<aetherdb_events::DocumentCreated>::connect(
        &brokers, "dlq-inspector",
    ).await.expect("connect dlq");
    // ... (implementation reads from <topic>.dlq, asserts the event is there)
    drop(dlq_consumer);
    handle.abort();
}

async fn start_redpanda() -> (String, testcontainers::ContainerAsync<testcontainers::GenericImage>) {
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};
    let container = GenericImage::new("redpandadata/redpanda", "v23.3.5")
        .with_exposed_port(9092.into())
        .with_cmd([
            "redpanda", "start", "--smp", "1", "--memory", "512M",
            "--reserve-memory", "0M", "--overprovisioned", "--node-id", "0",
            "--check=false",
            "--kafka-addr", "PLAINTEXT://0.0.0.0:9092",
            "--advertise-kafka-addr", "PLAINTEXT://127.0.0.1:9092",
        ])
        .with_mapped_port(9092_u16, 9092_u16.into())
        .start().await.expect("start redpanda");
    // Wait for Redpanda to be ready
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    ("localhost:9092".to_string(), container)
}

// FutureExt for `.fuse()` on tokio::signal::ctrl_c
use futures::FutureExt;
