use axum::http::HeaderValue;
use axum::middleware::Next;

pub async fn request_id_middleware(
    mut req: axum::extract::Request,
    next: Next,
) -> axum::response::Response {
    let request_id = uuid::Uuid::now_v7().to_string();
    if let Ok(val) = HeaderValue::from_str(&request_id) {
        req.headers_mut().insert("x-request-id", val.clone());
        let mut resp = next.run(req).await;
        resp.headers_mut().insert("x-request-id", val);
        resp
    } else {
        next.run(req).await
    }
}
