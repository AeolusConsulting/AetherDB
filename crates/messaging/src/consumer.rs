use std::future::Future;
use std::marker::PhantomData;
use std::time::Duration;

use rdkafka::Message;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};

use aetherdb_events::{Envelope, Event};

use crate::{HandlerError, MessagingError};

pub struct EventConsumer<E: Event> {
    consumer: StreamConsumer,
    _marker: PhantomData<E>,
}

impl<E: Event> EventConsumer<E>
where
    E::Payload: Clone + Send + Sync + 'static,
{
    pub async fn connect(brokers: &str, group_id: &str) -> Result<Self, MessagingError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "false")
            .set("isolation.level", "read_committed")
            .set("auto.offset.reset", "earliest")
            .create()
            .map_err(|e| MessagingError::Connection(e.to_string()))?;

        consumer
            .subscribe(&[E::TOPIC])
            .map_err(|e| MessagingError::Connection(e.to_string()))?;

        Ok(EventConsumer {
            consumer,
            _marker: PhantomData,
        })
    }

    pub async fn run<F, Fut, S>(self, handler: F, shutdown: S) -> Result<(), MessagingError>
    where
        F: Fn(E::Payload, Envelope<E::Payload>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), HandlerError>> + Send,
        S: Future + Send,
    {
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    tracing::info!("consumer shutting down");
                    return Ok(());
                }
                msg_result = self.consumer.recv() => {
                    let msg = msg_result.map_err(|e| MessagingError::Consume(e.to_string()))?;
                    let payload_bytes = match msg.payload() {
                        Some(p) => p,
                        None => continue,
                    };

                    let envelope: Envelope<E::Payload> = match serde_json::from_slice(payload_bytes) {
                        Ok(e) => e,
                        Err(e) => {
                            tracing::warn!("failed to deserialize message: {}", e);
                            self.consumer.commit_message(&msg, CommitMode::Async)
                                .map_err(|e| MessagingError::Consume(e.to_string()))?;
                            continue;
                        }
                    };

                    let payload = envelope.payload.clone();
                    let mut attempts = 0u32;
                    let mut backoff = Duration::from_millis(100);
                    let max_backoff = Duration::from_secs(5);
                    let max_attempts = 10u32;

                    loop {
                        attempts += 1;
                        match handler(payload.clone(), envelope.clone()).await {
                            Ok(()) => break,
                            Err(HandlerError::Permanent(e)) => {
                                tracing::error!("permanent handler error: {}", e);
                                break;
                            }
                            Err(HandlerError::Retryable(e)) => {
                                if attempts >= max_attempts {
                                    tracing::error!(
                                        "exhausted {} retries: {}",
                                        max_attempts, e
                                    );
                                    break;
                                }
                                tracing::warn!(
                                    "retryable error (attempt {}): {}",
                                    attempts, e
                                );
                                tokio::time::sleep(backoff).await;
                                backoff = (backoff * 2).min(max_backoff);
                            }
                        }
                    }

                    self.consumer.commit_message(&msg, CommitMode::Async)
                        .map_err(|e| MessagingError::Consume(e.to_string()))?;
                }
            }
        }
    }
}
