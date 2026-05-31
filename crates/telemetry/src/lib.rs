use std::net::SocketAddr;
use std::sync::{Mutex, OnceLock};

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub service_name: String,
    pub log_level: String,
    pub otlp_endpoint: Option<String>,
    pub prometheus_bind: SocketAddr,
}

pub struct TelemetryGuard {
    _shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(tx) = self._shutdown.take() {
            let _ = tx.send(());
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TelemetryError {
    #[error("failed to initialize tracing: {0}")]
    Tracing(String),
    #[error("failed to initialize metrics: {0}")]
    Metrics(String),
}

static HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());

const HISTOGRAM_BUCKETS: &[f64] = &[
    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

fn get_or_init_handle() -> Result<&'static PrometheusHandle, TelemetryError> {
    if let Some(h) = HANDLE.get() {
        return Ok(h);
    }
    let _lock = INIT_LOCK
        .lock()
        .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
    if let Some(h) = HANDLE.get() {
        return Ok(h);
    }
    let builder = PrometheusBuilder::new()
        .set_buckets(HISTOGRAM_BUCKETS)
        .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
    let handle = builder
        .install_recorder()
        .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
    let _ = HANDLE.set(handle);
    HANDLE
        .get()
        .ok_or_else(|| TelemetryError::Metrics("failed to store Prometheus handle".into()))
}

pub fn init(config: &TelemetryConfig) -> Result<TelemetryGuard, TelemetryError> {
    init_tracing(config);

    let handle = get_or_init_handle()?.clone();
    let bind = config.prometheus_bind;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
    let ready_clone = ready.clone();

    std::thread::spawn(move || {
        let Ok(rt) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            ready_clone.wait();
            return;
        };
        rt.block_on(async move {
            run_metrics_server(bind, handle, shutdown_rx, ready_clone).await;
        });
    });

    ready.wait();

    Ok(TelemetryGuard {
        _shutdown: Some(shutdown_tx),
    })
}

pub fn render_metrics() -> String {
    match HANDLE.get() {
        Some(h) => h.render(),
        None => String::new(),
    }
}

fn init_tracing(config: &TelemetryConfig) {
    let filter = EnvFilter::try_new(&config.log_level).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().json())
        .try_init();
}

async fn run_metrics_server(
    bind: SocketAddr,
    handle: PrometheusHandle,
    shutdown: tokio::sync::oneshot::Receiver<()>,
    ready: std::sync::Arc<std::sync::Barrier>,
) {
    let listener = match tokio::net::TcpListener::bind(bind).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("failed to bind metrics server on {}: {}", bind, e);
            ready.wait();
            return;
        }
    };

    ready.wait();

    tokio::select! {
        _ = async {
            loop {
                let (stream, _) = match listener.accept().await {
                    Ok(conn) => conn,
                    Err(_) => continue,
                };
                let h = handle.clone();
                tokio::spawn(async move {
                    handle_metrics_connection(stream, &h).await;
                });
            }
        } => {}
        _ = shutdown => {}
    }
}

async fn handle_metrics_connection(mut stream: tokio::net::TcpStream, handle: &PrometheusHandle) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; 4096];
    let _ = stream.read(&mut buf).await;

    let body = handle.render();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4; charset=utf-8\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    let _ = stream.write_all(response.as_bytes()).await;
}
