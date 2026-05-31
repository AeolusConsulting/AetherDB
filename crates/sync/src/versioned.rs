use aetherdb_domain::Document;

use crate::clock::{HlcTimestamp, NodeId};

#[derive(Debug, Clone)]
pub struct VersionedDocument {
    pub document: Document,
    pub hlc_ts: HlcTimestamp,
    pub origin_node: NodeId,
    pub deleted: bool,
}
