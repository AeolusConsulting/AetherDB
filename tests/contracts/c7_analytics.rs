// Contract C7 — analytics consumer
//
// Tests boot Redpanda + ClickHouse via testcontainers, then run the consumer
// in-process against them.

use std::time::Duration;

#[tokio::test]
#[ignore = "requires Docker (Redpanda + ClickHouse)"]
async fn each_documents_created_produces_one_clickhouse_row() {
    let stack = TestStack::boot().await;
    let producer = aetherdb_messaging::EventProducer::connect(&stack.brokers).await.unwrap();
    let consumer_handle = aetherdb_analytics_consumer::spawn(
        &stack.brokers, &stack.clickhouse_url, "test-group",
    ).await.unwrap();

    let mut doc_ids = Vec::new();
    for _ in 0..100 {
        let id = aetherdb_domain::DocumentId(uuid::Uuid::now_v7());
        doc_ids.push(id);
        producer.publish(&aetherdb_events::DocumentCreated {
            document_id: id,
            content_hash: "x".into(),
            content_length: 10,
            metadata: serde_json::json!({}),
        }).await.unwrap();
    }
    producer.flush(Duration::from_secs(5)).await.unwrap();

    // Wait up to 5s for all 100 rows to appear in ClickHouse.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        let count = clickhouse_count(&stack.clickhouse_url, "documents_analytics").await;
        if count >= 100 { break; }
        if std::time::Instant::now() > deadline {
            panic!("expected 100 rows in documents_analytics within 5s, got {count}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    consumer_handle.shutdown().await;
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn outage_recovery_loses_no_events() {
    let stack = TestStack::boot().await;
    let producer = aetherdb_messaging::EventProducer::connect(&stack.brokers).await.unwrap();
    let consumer_handle = aetherdb_analytics_consumer::spawn(
        &stack.brokers, &stack.clickhouse_url, "outage-test",
    ).await.unwrap();

    // 50 events, then pause ClickHouse, 50 more, then resume.
    for i in 0..50 {
        producer.publish(&aetherdb_events::DocumentCreated {
            document_id: aetherdb_domain::DocumentId(uuid::Uuid::now_v7()),
            content_hash: format!("{i}"),
            content_length: 1,
            metadata: serde_json::json!({}),
        }).await.unwrap();
    }
    producer.flush(Duration::from_secs(5)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await; // ingest baseline batch

    stack.pause_clickhouse().await;
    for i in 50..100 {
        producer.publish(&aetherdb_events::DocumentCreated {
            document_id: aetherdb_domain::DocumentId(uuid::Uuid::now_v7()),
            content_hash: format!("{i}"),
            content_length: 1,
            metadata: serde_json::json!({}),
        }).await.unwrap();
    }
    producer.flush(Duration::from_secs(5)).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5)).await; // consumer should pause
    stack.unpause_clickhouse().await;

    // Wait for the backlog to drain.
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        let count = clickhouse_count(&stack.clickhouse_url, "documents_analytics").await;
        if count >= 100 { break; }
        if std::time::Instant::now() > deadline {
            panic!("backlog did not drain after recovery, got {count}");
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    consumer_handle.shutdown().await;
}

// ---- helpers ----

struct TestStack {
    brokers: String,
    clickhouse_url: String,
    _redpanda: testcontainers::ContainerAsync<testcontainers::GenericImage>,
    clickhouse: testcontainers::ContainerAsync<testcontainers::GenericImage>,
}

impl TestStack {
    async fn boot() -> Self {
        use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

        let redpanda = GenericImage::new("redpandadata/redpanda", "v23.3.5")
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
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let clickhouse = GenericImage::new("clickhouse/clickhouse-server", "24.3")
            .with_exposed_port(8123.into())
            .with_mapped_port(8123_u16, 8123_u16.into())
            .with_env_var("CLICKHOUSE_DEFAULT_ACCESS_MANAGEMENT", "1")
            .with_env_var("CLICKHOUSE_PASSWORD", "")
            .start().await.expect("start clickhouse");
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        TestStack {
            brokers: "localhost:9092".into(),
            clickhouse_url: "http://localhost:8123".into(),
            _redpanda: redpanda,
            clickhouse,
        }
    }

    async fn pause_clickhouse(&self) {
        let id = self.clickhouse.id().to_string();
        let _ = tokio::process::Command::new("docker")
            .args(["pause", &id])
            .output().await;
    }

    async fn unpause_clickhouse(&self) {
        let id = self.clickhouse.id().to_string();
        let _ = tokio::process::Command::new("docker")
            .args(["unpause", &id])
            .output().await;
    }
}

async fn clickhouse_count(url: &str, table: &str) -> u64 {
    let query = format!("SELECT count() FROM {table}");
    let resp = reqwest::Client::new()
        .post(url)
        .body(query)
        .send().await;
    match resp {
        Ok(r) => {
            let text = r.text().await.unwrap_or_default();
            text.trim().parse().unwrap_or(0)
        }
        Err(_) => 0,
    }
}
