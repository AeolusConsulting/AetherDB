// Contract C8 — DuckDB exporter

use std::time::Duration;

#[tokio::test]
#[ignore = "requires duckdb cli on PATH and a populated libSQL file"]
async fn export_row_count_matches_source() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("aether.db");
    let libsql_url = format!("file:{}", db_path.display());

    // Seed 1000 docs.
    let storage = aetherdb_storage::LibsqlStorage::connect(&libsql_url, None, None, 1).await.expect("connect");
    storage.migrate().await.expect("migrate");
    for i in 0..1000 {
        storage.document_repo().insert(&aetherdb_domain::Document {
            id: aetherdb_domain::DocumentId(uuid::Uuid::now_v7()),
            content: format!("doc-{i}"),
            metadata: serde_json::json!({}),
            content_hash: aetherdb_domain::ContentHash([0;32]),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }).await.expect("insert");
    }
    drop(storage);

    // Run an export tick.
    let export_dir = tmp.path().join("exports");
    std::fs::create_dir_all(&export_dir).unwrap();
    aetherdb_duckdb_exporter::run_once(&libsql_url, &export_dir).await.expect("export");

    // Find the produced parquet file.
    let parquet = std::fs::read_dir(&export_dir).unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("documents_"))
        .expect("documents parquet exists");

    // Use DuckDB CLI to count rows.
    let output = std::process::Command::new("duckdb")
        .args(["-csv", "-c", &format!("SELECT COUNT(*) FROM read_parquet('{}')", parquet.path().display())])
        .output().expect("duckdb cli");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1000"), "expected 1000 in duckdb output, got: {stdout}");
}
