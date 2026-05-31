use axum::Json;

pub async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

pub async fn readiness() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}
