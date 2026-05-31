use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;

use aetherdb_domain::{ContentHash, Document};
use aetherdb_sync::{
    Change, HlcTimestamp, SyncPullRequest, SyncPullResponse, SyncPushRequest, SyncPushResponse,
    VersionedDocument,
};

use crate::state::AppState;

pub async fn sync_pull(
    State(state): State<AppState>,
    Json(req): Json<SyncPullRequest>,
) -> impl IntoResponse {
    if let Some(ref clock) = state.hlc_clock {
        let mut clock = clock.lock().await;
        clock.recv(req.since);
    }

    let changes = match state
        .storage
        .document_repo()
        .get_changes_since(&req.since)
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let server_hlc = if let Some(ref clock) = state.hlc_clock {
        clock.lock().await.now()
    } else {
        HlcTimestamp::ZERO
    };

    Json(SyncPullResponse {
        changes,
        server_hlc,
    })
    .into_response()
}

pub async fn sync_push(
    State(state): State<AppState>,
    Json(req): Json<SyncPushRequest>,
) -> impl IntoResponse {
    let mut accepted = 0usize;
    let mut rejected = 0usize;

    for change in &req.changes {
        if let Some(ref clock) = state.hlc_clock {
            clock.lock().await.recv(change.hlc_ts);
        }

        let local = state
            .storage
            .document_repo()
            .get_with_hlc(&change.document_id)
            .await
            .ok()
            .flatten();

        let should_accept = match local {
            None => true,
            Some(ref local_ver) => {
                let remote_ver = change_to_versioned(change);
                aetherdb_sync::merge_lww(local_ver, &remote_ver)
                    == aetherdb_sync::MergeResult::AcceptRemote
            }
        };

        if should_accept {
            let hash_bytes = hex_decode(&change.content_hash);
            let mut hash = [0u8; 32];
            let len = hash_bytes.len().min(32);
            hash[..len].copy_from_slice(&hash_bytes[..len]);

            let doc = Document {
                id: change.document_id,
                content: change.content.clone(),
                metadata: change.metadata.clone(),
                content_hash: ContentHash(hash),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };

            if let Err(e) = state
                .storage
                .document_repo()
                .upsert_with_hlc(&doc, &change.hlc_ts, &change.origin_node, change.deleted)
                .await
            {
                tracing::warn!("sync push upsert failed: {e}");
                rejected += 1;
                continue;
            }
            accepted += 1;
        } else {
            rejected += 1;
        }
    }

    let server_hlc = if let Some(ref clock) = state.hlc_clock {
        clock.lock().await.now()
    } else {
        HlcTimestamp::ZERO
    };

    Json(SyncPushResponse {
        accepted,
        rejected,
        server_hlc,
    })
    .into_response()
}

fn change_to_versioned(change: &Change) -> VersionedDocument {
    let hash_bytes = hex_decode(&change.content_hash);
    let mut hash = [0u8; 32];
    let len = hash_bytes.len().min(32);
    hash[..len].copy_from_slice(&hash_bytes[..len]);

    VersionedDocument {
        document: Document {
            id: change.document_id,
            content: change.content.clone(),
            metadata: change.metadata.clone(),
            content_hash: ContentHash(hash),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        hlc_ts: change.hlc_ts,
        origin_node: change.origin_node,
        deleted: change.deleted,
    }
}

fn hex_decode(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .filter_map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok()))
        .collect()
}
