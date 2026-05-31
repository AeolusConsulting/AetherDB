use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aetherdb_storage::LibsqlStorage;

use crate::routes;
use crate::state::AppState;

static DB_COUNTER: AtomicU64 = AtomicU64::new(0);

pub async fn spawn() -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
    let db_id = DB_COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_url = format!("file:testdb_{db_id}?mode=memory&cache=shared");

    let _ = aetherdb_telemetry::init(&aetherdb_telemetry::TelemetryConfig {
        service_name: "api-gateway-test".into(),
        log_level: "warn".into(),
        otlp_endpoint: None,
        prometheus_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
    });

    let storage = Arc::new(LibsqlStorage::connect(&db_url, None, None, 1).await?);
    storage.migrate().await?;
    let graph_querier = Arc::new(aetherdb_graph::GraphQuerier::new(storage.clone()));
    let state = AppState {
        storage,
        vector_searcher: None,
        graph_querier: Some(graph_querier),
        hnsw_index: None,
        producer: None,
        embed_provider: None,
        hlc_clock: None,
        embed_dim: 768,
    };

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    tokio::spawn(async move {
        axum::serve(listener, app).await.ok();
    });

    Ok(addr)
}

fn build_router(state: AppState) -> axum::Router {
    let router = routes::router(state);
    router.layer(axum::middleware::from_fn(request_id_middleware))
}

async fn request_id_middleware(
    mut req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let request_id = uuid::Uuid::now_v7().to_string();
    req.headers_mut().insert(
        "x-request-id",
        axum::http::HeaderValue::from_str(&request_id)
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("unknown")),
    );
    let mut resp = next.run(req).await;
    resp.headers_mut().insert(
        "x-request-id",
        axum::http::HeaderValue::from_str(&request_id)
            .unwrap_or_else(|_| axum::http::HeaderValue::from_static("unknown")),
    );
    resp
}
