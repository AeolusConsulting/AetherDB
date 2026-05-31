use std::time::Duration;

use rdkafka::Message;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use tokio::sync::oneshot;

pub struct ConsumerHandle {
    shutdown_tx: Option<oneshot::Sender<()>>,
    join: Option<tokio::task::JoinHandle<()>>,
}

impl ConsumerHandle {
    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(jh) = self.join.take() {
            let _ = jh.await;
        }
    }
}

pub async fn spawn(
    brokers: &str,
    clickhouse_url: &str,
    group_id: &str,
) -> Result<ConsumerHandle, anyhow::Error> {
    let client = reqwest::Client::new();
    let ch_url = clickhouse_url.to_string();

    create_tables(&client, &ch_url).await?;

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", group_id)
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .set("isolation.level", "read_committed")
        .create()?;

    consumer.subscribe(&["documents.created"])?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    let join = tokio::spawn(async move {
        tracing::info!("analytics-consumer started");
        run_consumer_loop(consumer, client, ch_url, shutdown_rx).await;
        tracing::info!("analytics-consumer shutting down");
    });

    Ok(ConsumerHandle {
        shutdown_tx: Some(shutdown_tx),
        join: Some(join),
    })
}

async fn create_tables(client: &reqwest::Client, ch_url: &str) -> Result<(), anyhow::Error> {
    let ddl = [
        "CREATE TABLE IF NOT EXISTS documents_analytics (
            event_id        UUID,
            event_type      LowCardinality(String),
            document_id     UUID,
            content_length  UInt32,
            timestamp       DateTime64(3, 'UTC')
        ) ENGINE = MergeTree
        ORDER BY (timestamp, event_type, document_id)",
        "CREATE TABLE IF NOT EXISTS embeddings_analytics (
            event_id        UUID,
            document_id     UUID,
            model           LowCardinality(String),
            dim             UInt16,
            duration_ms     UInt32,
            timestamp       DateTime64(3, 'UTC')
        ) ENGINE = MergeTree
        ORDER BY (timestamp, model, document_id)",
        "CREATE TABLE IF NOT EXISTS api_telemetry (
            request_id      UUID,
            method          LowCardinality(String),
            path            String,
            status          UInt16,
            duration_ms     UInt32,
            timestamp       DateTime64(3, 'UTC')
        ) ENGINE = MergeTree
        ORDER BY (timestamp, status)",
    ];

    for sql in ddl {
        let resp = client.post(ch_url).body(sql.to_string()).send().await?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("ClickHouse DDL failed: {body}");
        }
    }

    Ok(())
}

async fn run_consumer_loop(
    consumer: StreamConsumer,
    client: reqwest::Client,
    ch_url: String,
    shutdown: oneshot::Receiver<()>,
) {
    tokio::pin!(shutdown);

    let mut batch: Vec<RowData> = Vec::new();
    let mut batch_deadline = tokio::time::Instant::now() + Duration::from_secs(1);

    loop {
        let timeout = tokio::time::sleep_until(batch_deadline);
        tokio::pin!(timeout);

        tokio::select! {
            _ = &mut shutdown => {
                if !batch.is_empty() {
                    flush_batch(&client, &ch_url, &mut batch).await;
                }
                return;
            }
            _ = &mut timeout => {
                if !batch.is_empty() {
                    flush_batch(&client, &ch_url, &mut batch).await;
                }
                batch_deadline = tokio::time::Instant::now() + Duration::from_secs(1);
            }
            msg_result = consumer.recv() => {
                let msg = match msg_result {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("consumer recv error: {e}");
                        continue;
                    }
                };

                if let Some(payload) = msg.payload() {
                    if let Some(row) = parse_documents_created(payload) {
                        batch.push(row);
                    }
                }

                consumer.commit_message(&msg, CommitMode::Async).ok();

                if batch.len() >= 1000 {
                    flush_batch(&client, &ch_url, &mut batch).await;
                    batch_deadline = tokio::time::Instant::now() + Duration::from_secs(1);
                }
            }
        }
    }
}

struct RowData {
    event_id: String,
    event_type: String,
    document_id: String,
    content_length: u32,
    timestamp_ms: i64,
}

fn parse_documents_created(payload: &[u8]) -> Option<RowData> {
    let envelope: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let event_id = envelope.get("event_id")?.as_str()?.to_string();
    let event_type = envelope
        .get("event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("documents.created")
        .to_string();
    let timestamp_ms = envelope.get("timestamp")?.as_i64()?;
    let payload_obj = envelope.get("payload")?;
    let document_id = payload_obj.get("document_id")?.as_str()?.to_string();
    let content_length = payload_obj
        .get("content_length")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;

    Some(RowData {
        event_id,
        event_type,
        document_id,
        content_length,
        timestamp_ms,
    })
}

async fn flush_batch(client: &reqwest::Client, ch_url: &str, batch: &mut Vec<RowData>) {
    if batch.is_empty() {
        return;
    }

    let mut values = Vec::with_capacity(batch.len());
    for row in batch.iter() {
        let ts_sec = row.timestamp_ms as f64 / 1000.0;
        values.push(format!(
            "('{}', '{}', '{}', {}, {ts_sec})",
            row.event_id, row.event_type, row.document_id, row.content_length,
        ));
    }

    let sql = format!(
        "INSERT INTO documents_analytics (event_id, event_type, document_id, content_length, timestamp) VALUES {}",
        values.join(",")
    );

    let max_retries = 5u32;
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        match client.post(ch_url).body(sql.clone()).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!("flushed {} rows to documents_analytics", batch.len());
                batch.clear();
                return;
            }
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                tracing::warn!("ClickHouse insert failed (attempt {attempt}): {body}");
            }
            Err(e) => {
                tracing::warn!("ClickHouse insert error (attempt {attempt}): {e}");
            }
        }

        if attempt >= max_retries {
            tracing::error!("exhausted {max_retries} retries, pausing consumer");
            metrics::gauge!("clickhouse_consumer_paused").set(1.0);
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                match client.post(ch_url).body("SELECT 1").send().await {
                    Ok(r) if r.status().is_success() => {
                        tracing::info!("ClickHouse recovered, resuming");
                        metrics::gauge!("clickhouse_consumer_paused").set(0.0);
                        attempt = 0;
                        break;
                    }
                    _ => continue,
                }
            }
        }

        let backoff = Duration::from_millis(200 * 2u64.pow(attempt.min(4) - 1));
        tokio::time::sleep(backoff).await;
    }
}
