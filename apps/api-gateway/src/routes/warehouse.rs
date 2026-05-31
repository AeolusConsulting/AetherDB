use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;

use aetherdb_domain::{ContentHash, CreateDocumentResponse, Document, DocumentId};
use sha2::{Digest, Sha256};

use crate::state::AppState;

// --- Bulk Import ---

#[derive(serde::Deserialize)]
pub struct BulkImportItem {
    pub content: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

pub async fn bulk_import(
    State(state): State<AppState>,
    Json(items): Json<Vec<BulkImportItem>>,
) -> impl IntoResponse {
    if items.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "empty batch"})),
        )
            .into_response();
    }
    if items.len() > 1000 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "max 1000 items per batch"})),
        )
            .into_response();
    }

    let mut created = Vec::new();
    let mut errors = Vec::new();

    for (i, item) in items.iter().enumerate() {
        if item.content.is_empty() {
            errors.push(serde_json::json!({"index": i, "error": "empty content"}));
            continue;
        }
        if item.content.len() > 1_048_576 {
            errors.push(serde_json::json!({"index": i, "error": "content too large"}));
            continue;
        }

        let metadata = if item.metadata.is_null() || item.metadata.is_object() {
            if item.metadata.is_null() {
                serde_json::json!({})
            } else {
                item.metadata.clone()
            }
        } else {
            errors.push(serde_json::json!({"index": i, "error": "metadata must be object"}));
            continue;
        };

        let id = DocumentId(uuid::Uuid::now_v7());
        let now = chrono::Utc::now();
        let mut hasher = Sha256::new();
        hasher.update(item.content.as_bytes());
        let hash: [u8; 32] = hasher.finalize().into();

        let doc = Document {
            id,
            content: item.content.clone(),
            metadata,
            content_hash: ContentHash(hash),
            created_at: now,
            updated_at: now,
        };

        match state.storage.document_repo().insert(&doc).await {
            Ok(()) => {
                created.push(CreateDocumentResponse {
                    id: doc.id,
                    created_at: doc.created_at,
                });
            }
            Err(e) => {
                errors.push(serde_json::json!({"index": i, "error": e.to_string()}));
            }
        }
    }

    let resp = serde_json::json!({
        "created": created.len(),
        "errors": errors.len(),
        "documents": created,
        "error_details": errors,
    });

    (StatusCode::OK, Json(resp)).into_response()
}

// --- List / Filter ---

#[derive(serde::Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
    pub metadata_filter: Option<String>,
    pub content_contains: Option<String>,
    pub created_after: Option<i64>,
    pub created_before: Option<i64>,
}

fn default_limit() -> u32 {
    50
}

pub async fn list_documents(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let limit = params.limit.min(1000);
    let offset = params.offset;

    let mut conditions = vec!["deleted = 0".to_string()];
    let mut bind_values: Vec<String> = Vec::new();

    if let Some(ref filter) = params.content_contains {
        bind_values.push(format!("%{filter}%"));
        conditions.push(format!("content LIKE ?{}", bind_values.len()));
    }
    if let Some(after) = params.created_after {
        bind_values.push(after.to_string());
        conditions.push(format!("created_at >= ?{}", bind_values.len()));
    }
    if let Some(before) = params.created_before {
        bind_values.push(before.to_string());
        conditions.push(format!("created_at <= ?{}", bind_values.len()));
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, content, metadata, created_at, updated_at FROM documents WHERE {where_clause} ORDER BY created_at DESC LIMIT {limit} OFFSET {offset}"
    );

    let count_sql = format!("SELECT COUNT(*) as total FROM documents WHERE {where_clause}");

    let mut rows = match state.storage.conn().query(&sql, ()).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let mut documents: Vec<serde_json::Value> = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let id: String = row.get(0).unwrap_or_default();
        let content: String = row.get(1).unwrap_or_default();
        let metadata_str: String = row.get(2).unwrap_or_default();
        let created_ms: i64 = row.get(3).unwrap_or_default();
        let updated_ms: i64 = row.get(4).unwrap_or_default();

        documents.push(serde_json::json!({
            "id": id,
            "content": content,
            "metadata": serde_json::from_str::<serde_json::Value>(&metadata_str).unwrap_or_default(),
            "created_at": chrono::DateTime::from_timestamp_millis(created_ms),
            "updated_at": chrono::DateTime::from_timestamp_millis(updated_ms),
        }));
    }

    let mut count_rows = match state.storage.conn().query(&count_sql, ()).await {
        Ok(r) => r,
        Err(_) => {
            return Json(serde_json::json!({
                "documents": documents,
                "total": documents.len(),
                "limit": limit,
                "offset": offset,
            }))
            .into_response();
        }
    };
    let total: i64 = if let Ok(Some(row)) = count_rows.next().await {
        row.get(0).unwrap_or_default()
    } else {
        0
    };

    Json(serde_json::json!({
        "documents": documents,
        "total": total,
        "limit": limit,
        "offset": offset,
    }))
    .into_response()
}

// --- SQL Query ---

#[derive(serde::Deserialize)]
pub struct SqlQueryRequest {
    pub sql: String,
}

pub async fn sql_query(
    State(state): State<AppState>,
    Json(req): Json<SqlQueryRequest>,
) -> impl IntoResponse {
    let sql_lower = req.sql.trim().to_lowercase();

    if !sql_lower.starts_with("select") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "only SELECT queries allowed"})),
        )
            .into_response();
    }

    if sql_lower.contains("drop ")
        || sql_lower.contains("delete ")
        || sql_lower.contains("insert ")
        || sql_lower.contains("update ")
        || sql_lower.contains("alter ")
        || sql_lower.contains("create ")
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({"error": "write operations not allowed via SQL query endpoint"}),
            ),
        )
            .into_response();
    }

    let mut rows = match state.storage.conn().query(&req.sql, ()).await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let mut results: Vec<serde_json::Value> = Vec::new();
    while let Ok(Some(row)) = rows.next().await {
        let col_count = row.column_count();
        let mut obj = serde_json::Map::new();
        for i in 0..col_count {
            let name = row.column_name(i).unwrap_or("?").to_string();
            let val = row.get_value(i).ok();
            let json_val = match val {
                Some(libsql::Value::Text(s)) => serde_json::Value::String(s),
                Some(libsql::Value::Integer(n)) => serde_json::json!(n),
                Some(libsql::Value::Real(f)) => serde_json::json!(f),
                Some(libsql::Value::Null) => serde_json::Value::Null,
                Some(libsql::Value::Blob(_)) => serde_json::Value::String("[blob]".into()),
                None => serde_json::Value::Null,
            };
            obj.insert(name, json_val);
        }
        results.push(serde_json::Value::Object(obj));
    }

    Json(serde_json::json!({
        "rows": results,
        "count": results.len(),
    }))
    .into_response()
}
