use std::sync::Arc;

use aetherdb_config::{Config, LlmProviderKind};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env()?;
    let _guard = aetherdb_telemetry::init(&cfg.telemetry_config("graph-worker"))?;

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

    let provider: Arc<dyn aetherdb_graph_worker::LlmProvider> = match cfg.llm_provider {
        LlmProviderKind::Ollama => Arc::new(
            aetherdb_graph_worker::providers::ollama::OllamaLlmProvider::new(
                &cfg.llm_base_url,
                &cfg.llm_model,
            ),
        ),
        LlmProviderKind::OpenAi => Arc::new(
            aetherdb_graph_worker::providers::openai::OpenAiLlmProvider::new(
                &cfg.llm_base_url,
                &cfg.llm_model,
                cfg.llm_api_key.as_deref().unwrap_or(""),
            ),
        ),
    };

    let worker = Arc::new(aetherdb_graph_worker::Worker::new(
        provider,
        storage.clone(),
    ));

    tracing::info!("graph-worker ready");
    println!("graph-worker ready");

    if cfg.minimal_mode || !cfg.graph_enabled {
        tokio::signal::ctrl_c().await?;
    } else {
        let consumer =
            aetherdb_messaging::EventConsumer::<aetherdb_events::DocumentCreated>::connect(
                &cfg.redpanda_brokers,
                "graph-worker",
            )
            .await?;

        let s = storage.clone();
        consumer
            .run(
                move |payload, _env| {
                    let w = worker.clone();
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
                        w.process_document(payload.document_id, &content).await;
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
