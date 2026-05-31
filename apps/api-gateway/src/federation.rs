use std::sync::Arc;
use std::time::Duration;

use aetherdb_storage::LibsqlStorage;
use aetherdb_sync::{
    HlcTimestamp, HybridClock, NodeId, StdClock, SyncPullRequest, SyncPullResponse,
    SyncPushRequest, SyncPushResponse,
};

pub async fn run_spoke_sync(
    hub_url: String,
    storage: Arc<LibsqlStorage>,
    clock: Arc<tokio::sync::Mutex<HybridClock<StdClock>>>,
    node_id: NodeId,
    interval: Duration,
) {
    let client = reqwest::Client::new();
    let mut last_push_hlc = HlcTimestamp::ZERO;
    let mut last_pull_hlc = HlcTimestamp::ZERO;

    tracing::info!(
        "federation spoke started, hub={}, interval={}s",
        hub_url,
        interval.as_secs()
    );

    loop {
        tokio::time::sleep(interval).await;

        if let Err(e) = push_to_hub(
            &client,
            &hub_url,
            &storage,
            &clock,
            node_id,
            &mut last_push_hlc,
        )
        .await
        {
            tracing::warn!("federation push failed: {e}");
        }

        if let Err(e) = pull_from_hub(
            &client,
            &hub_url,
            &storage,
            &clock,
            node_id,
            &mut last_pull_hlc,
        )
        .await
        {
            tracing::warn!("federation pull failed: {e}");
        }
    }
}

async fn push_to_hub(
    client: &reqwest::Client,
    hub_url: &str,
    storage: &LibsqlStorage,
    clock: &tokio::sync::Mutex<HybridClock<StdClock>>,
    node_id: NodeId,
    last_push_hlc: &mut HlcTimestamp,
) -> Result<(), String> {
    let changes = storage
        .document_repo()
        .get_changes_since(last_push_hlc)
        .await
        .map_err(|e| e.to_string())?;

    if changes.is_empty() {
        return Ok(());
    }

    let req = SyncPushRequest { changes, node_id };

    let resp: SyncPushResponse = client
        .post(format!("{hub_url}/v1/sync/push"))
        .json(&req)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    *last_push_hlc = resp.server_hlc;
    clock.lock().await.recv(resp.server_hlc);

    tracing::info!(
        "federation push: accepted={}, rejected={}",
        resp.accepted,
        resp.rejected
    );
    Ok(())
}

async fn pull_from_hub(
    client: &reqwest::Client,
    hub_url: &str,
    storage: &LibsqlStorage,
    clock: &tokio::sync::Mutex<HybridClock<StdClock>>,
    node_id: NodeId,
    last_pull_hlc: &mut HlcTimestamp,
) -> Result<(), String> {
    let req = SyncPullRequest {
        since: *last_pull_hlc,
        node_id,
    };

    let resp: SyncPullResponse = client
        .post(format!("{hub_url}/v1/sync/pull"))
        .json(&req)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    if resp.changes.is_empty() {
        *last_pull_hlc = resp.server_hlc;
        return Ok(());
    }

    let mut applied = 0usize;
    for change in &resp.changes {
        clock.lock().await.recv(change.hlc_ts);

        let local = storage
            .document_repo()
            .get_with_hlc(&change.document_id)
            .await
            .ok()
            .flatten();

        let should_accept = match local {
            None => true,
            Some(ref local_ver) => {
                let remote_ver = aetherdb_sync::VersionedDocument {
                    document: aetherdb_domain::Document {
                        id: change.document_id,
                        content: change.content.clone(),
                        metadata: change.metadata.clone(),
                        content_hash: aetherdb_domain::ContentHash([0; 32]),
                        created_at: chrono::Utc::now(),
                        updated_at: chrono::Utc::now(),
                    },
                    hlc_ts: change.hlc_ts,
                    origin_node: change.origin_node,
                    deleted: change.deleted,
                };
                aetherdb_sync::merge_lww(local_ver, &remote_ver)
                    == aetherdb_sync::MergeResult::AcceptRemote
            }
        };

        if should_accept {
            let doc = aetherdb_domain::Document {
                id: change.document_id,
                content: change.content.clone(),
                metadata: change.metadata.clone(),
                content_hash: aetherdb_domain::ContentHash([0; 32]),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            };
            if storage
                .document_repo()
                .upsert_with_hlc(&doc, &change.hlc_ts, &change.origin_node, change.deleted)
                .await
                .is_ok()
            {
                applied += 1;
            }
        }
    }

    *last_pull_hlc = resp.server_hlc;
    tracing::info!(
        "federation pull: received={}, applied={}",
        resp.changes.len(),
        applied
    );
    Ok(())
}
