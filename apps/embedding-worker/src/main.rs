use std::sync::Arc;

use aetherdb_config::{Config, EmbedProviderKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env()?;
    let _guard = aetherdb_telemetry::init(&cfg.telemetry_config("embedding-worker"))?;

    let storage = Arc::new(
        aetherdb_storage::LibsqlStorage::connect(
            &cfg.libsql_url,
            cfg.libsql_auth_token.as_deref(),
            cfg.replica_local_path.as_deref(),
            cfg.sync_interval_secs,
        )
        .await?,
    );

    if cfg.storage_role == aetherdb_config::StorageRole::Primary {
        storage.migrate().await?;
    } else {
        storage.sync().await?;
    }

    let provider: Arc<dyn aetherdb_embedding_worker::EmbeddingProvider> = match cfg.embed_provider {
        EmbedProviderKind::Ollama => Arc::new(
            aetherdb_embedding_worker::providers::ollama::OllamaProvider::new(
                &cfg.embed_base_url,
                &cfg.embed_model,
                cfg.embed_dim,
            ),
        ),
        EmbedProviderKind::OpenAi => Arc::new(
            aetherdb_embedding_worker::providers::openai::OpenAiProvider::new(
                &cfg.embed_base_url,
                &cfg.embed_model,
                cfg.embed_dim,
                cfg.embed_api_key.as_deref().unwrap_or(""),
            ),
        ),
    };

    let worker = Arc::new(aetherdb_embedding_worker::Worker::new(
        provider,
        storage.clone(),
        cfg.embed_dim,
    ));
    tracing::info!("embedding-worker ready");
    println!("embedding-worker ready");

    if cfg.minimal_mode {
        tokio::signal::ctrl_c().await?;
    } else {
        let consumer =
            aetherdb_messaging::EventConsumer::<aetherdb_events::DocumentCreated>::connect(
                &cfg.redpanda_brokers,
                "embedding-worker",
            )
            .await?;

        let w = worker.clone();
        let s = storage.clone();
        consumer
            .run(
                move |payload, _env| {
                    let w = w.clone();
                    let s = s.clone();
                    async move {
                        let content = s
                            .document_repo()
                            .get(&payload.document_id)
                            .await
                            .ok()
                            .flatten()
                            .map(|d| d.content)
                            .unwrap_or_default();
                        let batch = vec![(payload.document_id, content)];
                        w.process_batch(&batch).await;
                        Ok(())
                    }
                },
                shutdown_signal(),
            )
            .await?;
    }

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("shutdown signal received");
}
