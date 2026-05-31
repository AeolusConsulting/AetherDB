use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = aetherdb_config::Config::from_env()?;
    let _guard = aetherdb_telemetry::init(&cfg.telemetry_config("api-gateway"))?;

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
        tracing::info!("replica mode: skipping migrations, performing initial sync");
        storage.sync().await?;
    }

    let vector_searcher = Arc::new(aetherdb_vector::VectorSearcher::new(
        storage.clone(),
        cfg.embed_dim,
    ));

    let graph_querier = if cfg.graph_enabled {
        Some(Arc::new(aetherdb_graph::GraphQuerier::new(storage.clone())))
    } else {
        None
    };

    let hnsw_index = if cfg.hnsw_enabled {
        let index = Arc::new(aetherdb_hnsw::HnswIndex::new(
            aetherdb_hnsw::HnswConfig {
                ef_construction: cfg.hnsw_ef_construction,
                m: cfg.hnsw_m,
                ef_search: cfg.hnsw_ef_search,
            },
            cfg.embed_dim,
        ));
        if let Err(e) = index.build_from_storage(&storage).await {
            tracing::warn!("HNSW build failed (will retry): {e}");
        }
        Some(index)
    } else {
        None
    };

    let producer = if !cfg.minimal_mode {
        match aetherdb_messaging::EventProducer::connect(&cfg.redpanda_brokers).await {
            Ok(p) => Some(Arc::new(p)),
            Err(e) => {
                tracing::warn!("failed to connect event producer: {e}");
                None
            }
        }
    } else {
        None
    };

    let embed_provider: Option<Arc<dyn aetherdb_embedding_worker::EmbeddingProvider>> =
        match cfg.embed_provider {
            aetherdb_config::EmbedProviderKind::Ollama => Some(Arc::new(
                aetherdb_embedding_worker::providers::ollama::OllamaProvider::new(
                    &cfg.embed_base_url,
                    &cfg.embed_model,
                    cfg.embed_dim,
                ),
            )),
            aetherdb_config::EmbedProviderKind::OpenAi => Some(Arc::new(
                aetherdb_embedding_worker::providers::openai::OpenAiProvider::new(
                    &cfg.embed_base_url,
                    &cfg.embed_model,
                    cfg.embed_dim,
                    cfg.embed_api_key.as_deref().unwrap_or(""),
                ),
            )),
        };

    let hlc_clock = {
        let node_id = aetherdb_sync::NodeId(uuid::Uuid::now_v7());
        let clock = aetherdb_sync::HybridClock::new(node_id, aetherdb_sync::StdClock);
        Some(Arc::new(tokio::sync::Mutex::new(clock)))
    };

    let state = aetherdb_api_gateway::state::AppState {
        storage,
        vector_searcher: Some(vector_searcher),
        graph_querier,
        hnsw_index,
        producer,
        embed_provider,
        hlc_clock,
        embed_dim: cfg.embed_dim,
    };

    let app = aetherdb_api_gateway::routes::router(state)
        .layer(axum::middleware::from_fn(
            aetherdb_api_gateway::middleware::auth::auth_middleware,
        ))
        .layer(axum::middleware::from_fn(
            aetherdb_api_gateway::middleware::request_id::request_id_middleware,
        ));
    let listener = tokio::net::TcpListener::bind(&cfg.gateway_bind).await?;
    tracing::info!("api-gateway ready on {}", cfg.gateway_bind);
    println!("api-gateway ready");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c().await.ok();
    tracing::info!("shutdown signal received");
}
