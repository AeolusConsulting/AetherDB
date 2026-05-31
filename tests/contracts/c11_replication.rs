// Contract C11 — distributed libSQL with embedded replicas.
//
// Tests use testcontainers to boot a sqld primary, then verify that
// an embedded replica can sync reads and delegate writes.

use std::sync::Arc;
use std::time::Duration;

use aetherdb_domain::{ContentHash, Document, DocumentId};
use aetherdb_storage::LibsqlStorage;

#[tokio::test]
#[ignore = "requires Docker (sqld)"]
async fn replica_reads_document_written_to_primary() {
    let (primary_url, _container) = start_sqld().await;

    let primary =
        LibsqlStorage::connect(&primary_url, None, None, 1)
            .await
            .expect("connect primary");
    primary.migrate().await.expect("migrate");
    assert_eq!(primary.mode(), aetherdb_storage::StorageMode::Local);

    let doc_id = DocumentId(uuid::Uuid::now_v7());
    let doc = Document {
        id: doc_id,
        content: "hello from primary".into(),
        metadata: serde_json::json!({}),
        content_hash: ContentHash([0; 32]),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    primary.document_repo().insert(&doc).await.expect("insert");

    let tmp = tempfile::tempdir().expect("tempdir");
    let replica_path = tmp.path().join("replica.db");
    let replica = LibsqlStorage::connect(
        &primary_url,
        None,
        Some(replica_path.to_str().expect("path")),
        1,
    )
    .await
    .expect("connect replica");
    assert_eq!(replica.mode(), aetherdb_storage::StorageMode::Replica);

    replica.sync().await.expect("initial sync");

    let got = replica
        .document_repo()
        .get(&doc_id)
        .await
        .expect("get from replica");
    assert!(got.is_some(), "replica should have the document after sync");
    assert_eq!(got.expect("doc").content, "hello from primary");
}

#[tokio::test]
#[ignore = "requires Docker (sqld)"]
async fn replica_write_delegates_to_primary() {
    let (primary_url, _container) = start_sqld().await;

    let primary =
        LibsqlStorage::connect(&primary_url, None, None, 1)
            .await
            .expect("connect primary");
    primary.migrate().await.expect("migrate");

    let tmp = tempfile::tempdir().expect("tempdir");
    let replica_path = tmp.path().join("replica.db");
    let replica = LibsqlStorage::connect(
        &primary_url,
        None,
        Some(replica_path.to_str().expect("path")),
        1,
    )
    .await
    .expect("connect replica");
    replica.sync().await.expect("initial sync");

    let doc_id = DocumentId(uuid::Uuid::now_v7());
    let doc = Document {
        id: doc_id,
        content: "written via replica".into(),
        metadata: serde_json::json!({}),
        content_hash: ContentHash([0; 32]),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    replica
        .document_repo()
        .insert(&doc)
        .await
        .expect("insert via replica");

    let got = replica
        .document_repo()
        .get(&doc_id)
        .await
        .expect("get from replica");
    assert!(
        got.is_some(),
        "replica should see its own write (read-your-writes)"
    );

    let got_primary = primary
        .document_repo()
        .get(&doc_id)
        .await
        .expect("get from primary");
    assert!(
        got_primary.is_some(),
        "primary should have the document written via replica"
    );
}

async fn start_sqld() -> (
    String,
    testcontainers::ContainerAsync<testcontainers::GenericImage>,
) {
    use testcontainers::{GenericImage, ImageExt, runners::AsyncRunner};

    let container = GenericImage::new("ghcr.io/tursodatabase/libsql-server", "latest")
        .with_exposed_port(8080.into())
        .with_mapped_port(8080_u16, 8080_u16.into())
        .start()
        .await
        .expect("start sqld");
    tokio::time::sleep(Duration::from_secs(3)).await;

    ("http://localhost:8080".to_string(), container)
}
