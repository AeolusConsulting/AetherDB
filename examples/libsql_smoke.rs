//! Smoke test for libSQL vector search.
//!
//! Invoked by `scripts/smoke-libsql-vector.sh`. This is a GATE: it must pass
//! before crates/vector is implemented. If it fails, the pinned `libsql`
//! crate version is unsuitable.
//!
//! Behaviour:
//!   1. Open in-memory libSQL.
//!   2. Create a table with a 4-dim FLOAT32 column + libsql_vector_idx.
//!   3. Insert 5 vectors.
//!   4. Run a vector_top_k JOIN.
//!   5. Print EXPLAIN QUERY PLAN.
//!
//! The bash wrapper inspects stdout for "VIRTUAL TABLE INDEX" and the absence
//! of "SCAN document_embeddings".

use anyhow::Result;
use libsql::{Builder, params};

const DIM: usize = 4;

#[tokio::main]
async fn main() -> Result<()> {
    let db = Builder::new_local(":memory:").build().await?;
    let conn = db.connect()?;

    // Schema matching what migrations/002_embeddings.sql.tmpl will render.
    conn.execute_batch(&format!(
        r#"
        CREATE TABLE documents (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL
        );
        CREATE TABLE document_embeddings (
            document_id TEXT NOT NULL UNIQUE REFERENCES documents(id) ON DELETE CASCADE,
            embedding   F32_BLOB({DIM}) NOT NULL
        );
        CREATE INDEX idx_embeddings_vector ON document_embeddings(
            libsql_vector_idx(embedding, 'metric=cosine')
        );
        "#
    )).await?;

    // Insert 5 documents + 5 vectors.
    for i in 0..5 {
        let doc_id = format!("doc-{i}");
        conn.execute(
            "INSERT INTO documents (id, content) VALUES (?1, ?2)",
            params![doc_id.clone(), format!("content {i}")],
        ).await?;
        let v = format!("[{}, {}, {}, {}]", i as f32, i as f32 + 0.1, i as f32 + 0.2, i as f32 + 0.3);
        conn.execute(
            "INSERT INTO document_embeddings (document_id, embedding) VALUES (?1, vector32(?2))",
            params![doc_id, v],
        ).await?;
    }

    // The canonical query shape from CONTRACTS.md § C4.2.
    let query_v = "[0.0, 0.1, 0.2, 0.3]";
    let sql = r#"
        SELECT
            de.document_id                                                 AS document_id,
            vector_distance_cos(de.embedding, vector32(?1))                AS distance
        FROM vector_top_k('idx_embeddings_vector', vector32(?1), ?2) AS vt
        JOIN document_embeddings AS de ON de.rowid = vt.id
        ORDER BY distance ASC
    "#;

    println!("==> Running vector_top_k query...");
    let mut rows = conn.query(sql, params![query_v, 3i64]).await?;
    let mut row_count = 0;
    while let Some(row) = rows.next().await? {
        let doc_id: String = row.get(0)?;
        let distance: f64 = row.get(1)?;
        println!("  match: {doc_id} (distance={distance:.6})");
        row_count += 1;
    }
    if row_count == 0 {
        anyhow::bail!("vector_top_k returned zero rows — index may not be configured correctly");
    }
    println!("==> Got {row_count} rows.");

    println!("==> EXPLAIN QUERY PLAN:");
    let explain_sql = format!("EXPLAIN QUERY PLAN {sql}");
    let mut plan = conn.query(&explain_sql, params![query_v, 3i64]).await?;
    let mut plan_string = String::new();
    while let Some(row) = plan.next().await? {
        let detail: String = row.get(3)?;
        println!("  {detail}");
        plan_string.push_str(&detail);
        plan_string.push('\n');
    }

    if !plan_string.contains("VIRTUAL TABLE INDEX") {
        anyhow::bail!("plan does not show VIRTUAL TABLE INDEX — vector index not used");
    }
    if plan_string.contains("SCAN document_embeddings") {
        anyhow::bail!("plan shows full SCAN document_embeddings — index not consulted");
    }

    println!("\nOK: vector search works end-to-end and uses the index.");
    Ok(())
}
