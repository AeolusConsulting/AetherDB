use std::path::Path;

pub async fn run_once(libsql_url: &str, export_dir: &Path) -> Result<(), anyhow::Error> {
    let storage = aetherdb_storage::LibsqlStorage::connect(libsql_url, None, None, 1).await?;
    storage.migrate().await?;

    let timestamp = chrono::Utc::now().timestamp();

    let docs_path = export_dir.join(format!("documents_{timestamp}.parquet"));

    let mut rows = storage
        .conn()
        .query(
            "SELECT id, content, metadata, created_at, updated_at FROM documents",
            (),
        )
        .await?;

    let db = duckdb::Connection::open_in_memory()?;
    db.execute_batch(
        "CREATE TABLE documents_export (
            id VARCHAR,
            content VARCHAR,
            metadata VARCHAR,
            created_at BIGINT,
            updated_at BIGINT
        )",
    )?;

    let mut count = 0u64;
    while let Some(row) = rows.next().await? {
        let id: String = row.get(0)?;
        let content: String = row.get(1)?;
        let metadata: String = row.get(2)?;
        let created_at: i64 = row.get(3)?;
        let updated_at: i64 = row.get(4)?;
        db.execute(
            "INSERT INTO documents_export VALUES (?, ?, ?, ?, ?)",
            duckdb::params![id, content, metadata, created_at, updated_at],
        )?;
        count += 1;
    }

    db.execute(
        &format!(
            "COPY documents_export TO '{}' (FORMAT PARQUET)",
            docs_path.display()
        ),
        [],
    )?;

    tracing::info!("exported {count} documents to {}", docs_path.display());

    Ok(())
}
