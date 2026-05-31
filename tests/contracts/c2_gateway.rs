// Contract C2 — API gateway
//
// These tests boot the gateway in-process with stubbed dependencies (in-memory
// libSQL + a mock event producer) and exercise HTTP endpoints with reqwest.

use std::net::SocketAddr;
use std::time::Duration;

const REQUIRED_ENDPOINTS: &[(&str, &str)] = &[
    ("GET", "/health"),
    ("GET", "/readiness"),
    ("GET", "/metrics"),
    ("POST", "/v1/documents"),
    // GET/DELETE /v1/documents/:id covered separately
    ("POST", "/v1/search/semantic"),
];

async fn boot_gateway() -> SocketAddr {
    // The gateway exposes a `test_harness::spawn()` that returns the bound addr
    // and a shutdown handle. Tests use this to avoid spinning up the full stack.
    let addr = aetherdb_api_gateway::test_harness::spawn().await.expect("spawn gateway");
    // Brief wait for socket to be ready.
    tokio::time::sleep(Duration::from_millis(50)).await;
    addr
}

#[tokio::test]
async fn health_endpoint_returns_200_ok() {
    let addr = boot_gateway().await;
    let resp = reqwest::get(format!("http://{addr}/health")).await.expect("get");
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.expect("json");
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn create_document_returns_201_and_id() {
    let addr = boot_gateway().await;
    let client = reqwest::Client::new();
    let resp = client.post(format!("http://{addr}/v1/documents"))
        .json(&serde_json::json!({"content": "hello world", "metadata": {}}))
        .send().await.expect("send");
    assert_eq!(resp.status(), 201);
    let body: serde_json::Value = resp.json().await.expect("json");
    assert!(body.get("id").is_some());
    assert!(body.get("created_at").is_some());
    // Must be a valid UUID
    let id_str = body["id"].as_str().expect("id is string");
    uuid::Uuid::parse_str(id_str).expect("id is valid UUID");
}

#[tokio::test]
async fn create_document_with_empty_content_returns_400() {
    let addr = boot_gateway().await;
    let resp = reqwest::Client::new().post(format!("http://{addr}/v1/documents"))
        .json(&serde_json::json!({"content": ""}))
        .send().await.expect("send");
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn create_document_with_array_metadata_returns_400() {
    let addr = boot_gateway().await;
    let resp = reqwest::Client::new().post(format!("http://{addr}/v1/documents"))
        .json(&serde_json::json!({"content": "x", "metadata": [1, 2, 3]}))
        .send().await.expect("send");
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn get_document_after_create_returns_same_content() {
    let addr = boot_gateway().await;
    let client = reqwest::Client::new();
    let created: serde_json::Value = client.post(format!("http://{addr}/v1/documents"))
        .json(&serde_json::json!({"content": "roundtrip"}))
        .send().await.unwrap()
        .json().await.unwrap();
    let id = created["id"].as_str().unwrap();
    let got: serde_json::Value = client.get(format!("http://{addr}/v1/documents/{id}"))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(got["content"], "roundtrip");
    assert_eq!(got["id"], created["id"]);
}

#[tokio::test]
async fn get_nonexistent_document_returns_404() {
    let addr = boot_gateway().await;
    let missing = uuid::Uuid::now_v7();
    let resp = reqwest::get(format!("http://{addr}/v1/documents/{missing}")).await.expect("get");
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn delete_returns_204_then_get_returns_404() {
    let addr = boot_gateway().await;
    let client = reqwest::Client::new();
    let created: serde_json::Value = client.post(format!("http://{addr}/v1/documents"))
        .json(&serde_json::json!({"content": "to-delete"}))
        .send().await.unwrap().json().await.unwrap();
    let id = created["id"].as_str().unwrap();
    let del = client.delete(format!("http://{addr}/v1/documents/{id}")).send().await.unwrap();
    assert_eq!(del.status(), 204);
    let get = client.get(format!("http://{addr}/v1/documents/{id}")).send().await.unwrap();
    assert_eq!(get.status(), 404);
}

#[tokio::test]
async fn idempotency_key_replays_first_response() {
    let addr = boot_gateway().await;
    let client = reqwest::Client::new();
    let key = uuid::Uuid::now_v7().to_string();
    let first = client.post(format!("http://{addr}/v1/documents"))
        .header("Idempotency-Key", &key)
        .json(&serde_json::json!({"content": "idem"}))
        .send().await.unwrap();
    assert_eq!(first.status(), 201);
    let first_body: serde_json::Value = first.json().await.unwrap();

    let second = client.post(format!("http://{addr}/v1/documents"))
        .header("Idempotency-Key", &key)
        .json(&serde_json::json!({"content": "idem"}))
        .send().await.unwrap();
    assert_eq!(second.status(), 200, "replay must downgrade 201 to 200");
    let second_body: serde_json::Value = second.json().await.unwrap();
    assert_eq!(first_body["id"], second_body["id"], "replay must return same id");
}

#[tokio::test]
async fn request_id_header_is_set_and_echoed() {
    let addr = boot_gateway().await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/documents"))
        .json(&serde_json::json!({"content": "x"}))
        .send().await.unwrap();
    let rid = resp.headers().get("x-request-id").expect("X-Request-Id header missing");
    uuid::Uuid::parse_str(rid.to_str().unwrap()).expect("X-Request-Id must be a UUID");
}

#[tokio::test]
async fn search_with_top_k_out_of_range_returns_400() {
    let addr = boot_gateway().await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/search/semantic"))
        .json(&serde_json::json!({"query": "q", "top_k": 1000}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn search_response_includes_pending_embeddings_header() {
    let addr = boot_gateway().await;
    let resp = reqwest::Client::new()
        .post(format!("http://{addr}/v1/search/semantic"))
        .json(&serde_json::json!({"query": "q", "top_k": 5}))
        .send().await.unwrap();
    // Either 200 or 503 (no embeddings yet) — but if 200, header must be present.
    if resp.status() == 200 {
        assert!(resp.headers().get("x-pending-embeddings").is_some(),
            "successful search must include X-Pending-Embeddings header");
    }
}

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_exposition() {
    let addr = boot_gateway().await;
    let resp = reqwest::get(format!("http://{addr}/metrics")).await.expect("get");
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.expect("text");
    // Prometheus exposition format always has at least one HELP or TYPE line.
    assert!(body.contains("# HELP") || body.contains("# TYPE"),
        "expected Prometheus exposition format, got:\n{}", &body[..body.len().min(500)]);
    // Required metric per C9.2:
    assert!(body.contains("http_requests_total"));
}
