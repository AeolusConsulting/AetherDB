use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use std::sync::Arc;

#[derive(Clone)]
pub struct ApiKeys {
    keys: Arc<Vec<String>>,
}

impl ApiKeys {
    pub fn from_env() -> Self {
        let keys_str = std::env::var("AETHERDB_API_KEYS").unwrap_or_default();
        let keys: Vec<String> = keys_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        ApiKeys {
            keys: Arc::new(keys),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.keys.is_empty()
    }

    fn validate(&self, token: &str) -> bool {
        self.keys.iter().any(|k| k == token)
    }
}

pub async fn auth_middleware(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if path == "/health" || path == "/readiness" || path == "/metrics" {
        return next.run(req).await;
    }

    let keys = ApiKeys::from_env();
    if !keys.is_enabled() {
        return next.run(req).await;
    }

    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if keys.validate(token) {
                next.run(req).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({"error": "invalid API key", "code": "UNAUTHORIZED"})),
                )
                    .into_response()
            }
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(
                serde_json::json!({"error": "missing Authorization header", "code": "UNAUTHORIZED"}),
            ),
        )
            .into_response(),
    }
}
