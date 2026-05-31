use axum::response::IntoResponse;

pub async fn metrics() -> impl IntoResponse {
    metrics::counter!("http_requests_total", "method" => "GET", "path" => "/metrics", "status" => "200").increment(0);
    let body = aetherdb_telemetry::render_metrics();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
}
