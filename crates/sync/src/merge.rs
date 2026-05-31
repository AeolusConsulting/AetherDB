use crate::versioned::VersionedDocument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeResult {
    KeepLocal,
    AcceptRemote,
}

pub fn merge_lww(local: &VersionedDocument, remote: &VersionedDocument) -> MergeResult {
    match remote.hlc_ts.cmp(&local.hlc_ts) {
        std::cmp::Ordering::Greater => MergeResult::AcceptRemote,
        std::cmp::Ordering::Less => MergeResult::KeepLocal,
        std::cmp::Ordering::Equal => {
            if remote.origin_node > local.origin_node {
                MergeResult::AcceptRemote
            } else {
                MergeResult::KeepLocal
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::clock::{HlcTimestamp, NodeId};
    use aetherdb_domain::{ContentHash, Document, DocumentId};
    use uuid::Uuid;

    fn make_versioned(
        wall_ms: i64,
        counter: u32,
        node_byte: u8,
        deleted: bool,
    ) -> VersionedDocument {
        let mut node_bytes = [0u8; 16];
        node_bytes[0] = node_byte;
        VersionedDocument {
            document: Document {
                id: DocumentId(Uuid::now_v7()),
                content: "test".into(),
                metadata: serde_json::json!({}),
                content_hash: ContentHash([0; 32]),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            },
            hlc_ts: HlcTimestamp { wall_ms, counter },
            origin_node: NodeId(Uuid::from_bytes(node_bytes)),
            deleted,
        }
    }

    #[test]
    fn remote_newer_wins() {
        let local = make_versioned(1000, 0, 1, false);
        let remote = make_versioned(2000, 0, 2, false);
        assert_eq!(merge_lww(&local, &remote), MergeResult::AcceptRemote);
    }

    #[test]
    fn local_newer_wins() {
        let local = make_versioned(2000, 0, 1, false);
        let remote = make_versioned(1000, 0, 2, false);
        assert_eq!(merge_lww(&local, &remote), MergeResult::KeepLocal);
    }

    #[test]
    fn same_hlc_higher_node_wins() {
        let local = make_versioned(1000, 5, 1, false);
        let remote = make_versioned(1000, 5, 2, false);
        assert_eq!(merge_lww(&local, &remote), MergeResult::AcceptRemote);
    }

    #[test]
    fn same_hlc_lower_node_keeps_local() {
        let local = make_versioned(1000, 5, 2, false);
        let remote = make_versioned(1000, 5, 1, false);
        assert_eq!(merge_lww(&local, &remote), MergeResult::KeepLocal);
    }

    #[test]
    fn delete_beats_older_update() {
        let local = make_versioned(1000, 0, 1, false);
        let remote = make_versioned(2000, 0, 2, true);
        assert_eq!(merge_lww(&local, &remote), MergeResult::AcceptRemote);
    }

    #[test]
    fn update_beats_older_delete() {
        let local = make_versioned(1000, 0, 1, true);
        let remote = make_versioned(2000, 0, 2, false);
        assert_eq!(merge_lww(&local, &remote), MergeResult::AcceptRemote);
    }
}
