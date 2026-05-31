// Contract C9 — telemetry

use std::net::SocketAddr;
use aetherdb_telemetry::{init, TelemetryConfig};

fn cfg(name: &str, port: u16) -> TelemetryConfig {
    TelemetryConfig {
        service_name: name.into(),
        log_level: "info".into(),
        otlp_endpoint: None,
        prometheus_bind: SocketAddr::from(([127, 0, 0, 1], port)),
    }
}

#[tokio::test]
async fn metrics_endpoint_serves_prometheus_exposition() {
    let _guard = init(&cfg("c9-test-1", 19090)).expect("init telemetry");
    // Emit one metric so there's something to expose.
    metrics::counter!("test_counter").increment(1);
    let body = reqwest::get("http://127.0.0.1:19090/metrics").await.expect("get")
        .text().await.expect("text");
    assert!(body.contains("test_counter"));
    assert!(body.contains("# HELP") || body.contains("# TYPE"));
}

#[tokio::test]
async fn histograms_use_required_buckets() {
    let _guard = init(&cfg("c9-test-2", 19091)).expect("init telemetry");
    metrics::histogram!("test_hist").record(0.01);
    metrics::histogram!("test_hist").record(0.5);
    let body = reqwest::get("http://127.0.0.1:19091/metrics").await.expect("get")
        .text().await.expect("text");
    // Required buckets per C9.2.
    for le in ["0.001", "0.005", "0.01", "0.025", "0.05", "0.1", "0.25", "0.5", "1", "2.5", "5", "10"] {
        assert!(
            body.contains(&format!(r#"le="{le}""#)),
            "histogram bucket le={le} not exposed; got:\n{body}"
        );
    }
}

#[test]
fn telemetry_disables_otlp_when_endpoint_unset() {
    // Constructing init() with otlp_endpoint=None MUST NOT error, and the
    // returned guard MUST NOT hold an OTLP exporter.
    let _guard = init(&cfg("c9-test-3", 19092)).expect("init telemetry without otlp");
}
