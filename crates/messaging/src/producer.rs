use std::collections::HashMap;
use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};

use aetherdb_events::{Envelope, Event};

use crate::MessagingError;

pub struct ProducerConfig {
    settings: HashMap<String, String>,
}

impl ProducerConfig {
    pub fn get(&self, key: &str) -> Option<&String> {
        self.settings.get(key)
    }
}

pub fn producer_default_config(brokers: &str) -> ProducerConfig {
    let mut settings = HashMap::new();
    settings.insert("bootstrap.servers".into(), brokers.into());
    settings.insert("acks".into(), "all".into());
    settings.insert("enable.idempotence".into(), "true".into());
    settings.insert("max.in.flight.requests.per.connection".into(), "5".into());
    settings.insert("retries".into(), "10".into());
    settings.insert("compression.type".into(), "zstd".into());
    settings.insert("linger.ms".into(), "5".into());
    ProducerConfig { settings }
}

pub struct EventProducer {
    producer: FutureProducer,
}

impl EventProducer {
    pub async fn connect(brokers: &str) -> Result<Self, MessagingError> {
        let cfg = producer_default_config(brokers);
        let mut client_config = ClientConfig::new();
        for (k, v) in &cfg.settings {
            client_config.set(k, v);
        }
        let producer: FutureProducer = client_config
            .create()
            .map_err(|e| MessagingError::Connection(e.to_string()))?;
        Ok(EventProducer { producer })
    }

    pub async fn publish<E: Event>(&self, event: &E) -> Result<(), MessagingError>
    where
        E::Payload: Clone,
    {
        let envelope = Envelope {
            event_id: uuid::Uuid::now_v7(),
            event_type: E::TOPIC.to_string(),
            event_version: E::VERSION,
            timestamp: chrono::Utc::now().timestamp_millis(),
            source: "aetherdb".to_string(),
            trace_id: None,
            payload: event.payload().clone(),
        };

        let payload =
            serde_json::to_vec(&envelope).map_err(|e| MessagingError::Publish(e.to_string()))?;
        let key = event.key();

        let record = FutureRecord::to(E::TOPIC).key(&key).payload(&payload);

        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| MessagingError::Publish(e.to_string()))?;

        Ok(())
    }

    pub async fn flush(&self, timeout: Duration) -> Result<(), MessagingError> {
        self.producer
            .flush(timeout)
            .map_err(|e| MessagingError::Publish(e.to_string()))
    }
}
