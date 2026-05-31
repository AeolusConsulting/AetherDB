use aetherdb_domain::DocumentId;
use serde::{Deserialize, Serialize};

use crate::clock::{HlcTimestamp, NodeId};

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullRequest {
    pub since: HlcTimestamp,
    pub node_id: NodeId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPullResponse {
    pub changes: Vec<Change>,
    pub server_hlc: HlcTimestamp,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPushRequest {
    pub changes: Vec<Change>,
    pub node_id: NodeId,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncPushResponse {
    pub accepted: usize,
    pub rejected: usize,
    pub server_hlc: HlcTimestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub document_id: DocumentId,
    pub content: String,
    pub metadata: serde_json::Value,
    pub content_hash: String,
    pub hlc_ts: HlcTimestamp,
    pub origin_node: NodeId,
    pub deleted: bool,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn change_roundtrip_serde() {
        let change = Change {
            document_id: DocumentId(Uuid::now_v7()),
            content: "hello".into(),
            metadata: serde_json::json!({"key": "value"}),
            content_hash: "deadbeef".into(),
            hlc_ts: HlcTimestamp {
                wall_ms: 1000,
                counter: 5,
            },
            origin_node: NodeId(Uuid::now_v7()),
            deleted: false,
        };
        let json = serde_json::to_string(&change).unwrap();
        let back: Change = serde_json::from_str(&json).unwrap();
        assert_eq!(back.document_id, change.document_id);
        assert_eq!(back.hlc_ts.wall_ms, 1000);
        assert_eq!(back.hlc_ts.counter, 5);
        assert!(!back.deleted);
    }

    #[test]
    fn sync_request_response_serde() {
        let req = SyncPullRequest {
            since: HlcTimestamp::ZERO,
            node_id: NodeId(Uuid::now_v7()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let _: SyncPullRequest = serde_json::from_str(&json).unwrap();

        let resp = SyncPushResponse {
            accepted: 5,
            rejected: 2,
            server_hlc: HlcTimestamp {
                wall_ms: 2000,
                counter: 0,
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: SyncPushResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.accepted, 5);
    }
}
