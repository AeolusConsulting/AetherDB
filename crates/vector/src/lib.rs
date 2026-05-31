use std::sync::Arc;

pub use aetherdb_domain::VectorError;
use aetherdb_domain::{DocumentId, StorageError};
use aetherdb_storage::LibsqlStorage;

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub document_id: DocumentId,
    pub score: f32,
    pub distance: f32,
}

pub struct VectorSearcher {
    storage: Arc<LibsqlStorage>,
    embed_dim: u16,
}

impl VectorSearcher {
    pub fn new(storage: Arc<LibsqlStorage>, embed_dim: u16) -> Self {
        VectorSearcher { storage, embed_dim }
    }

    pub async fn search_similar(
        &self,
        query: &[f32],
        top_k: usize,
        min_score: Option<f32>,
    ) -> Result<Vec<SearchResult>, VectorError> {
        validate_query(query, self.embed_dim)?;
        validate_top_k(top_k)?;

        let vec_str = format!(
            "[{}]",
            query
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );

        let sql = r"
            SELECT
                de.document_id                                                 AS document_id,
                vector_distance_cos(de.embedding, vector32(?1))                AS distance
            FROM vector_top_k('idx_embeddings_vector', vector32(?1), ?2) AS vt
            JOIN document_embeddings AS de ON de.rowid = vt.id
            ORDER BY distance ASC
        ";

        let mut rows = self
            .storage
            .conn()
            .query(sql, libsql::params![vec_str.clone(), top_k as i64])
            .await
            .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?
        {
            let doc_id_str: String = row
                .get(0)
                .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?;
            let distance: f64 = row
                .get(1)
                .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?;

            let distance = distance as f32;
            let score = (1.0_f32 - distance).clamp(0.0, 1.0);

            if let Some(min) = min_score {
                if score < min {
                    continue;
                }
            }

            let uid = uuid::Uuid::parse_str(&doc_id_str)
                .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?;

            results.push(SearchResult {
                document_id: DocumentId(uid),
                score,
                distance,
            });
        }

        Ok(results)
    }
}

pub async fn debug_explain_search_plan(
    storage: &LibsqlStorage,
    _embed_dim: u16,
) -> Result<String, VectorError> {
    let dummy_vec = "[0.0,0.1,0.2,0.3]";

    let explain_sql = r"
        EXPLAIN QUERY PLAN
        SELECT
            de.document_id                                                 AS document_id,
            vector_distance_cos(de.embedding, vector32(?1))                AS distance
        FROM vector_top_k('idx_embeddings_vector', vector32(?1), ?2) AS vt
        JOIN document_embeddings AS de ON de.rowid = vt.id
        ORDER BY distance ASC
    ";

    let mut rows = storage
        .conn()
        .query(explain_sql, libsql::params![dummy_vec, 5i64])
        .await
        .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?;

    let mut plan = String::new();
    while let Some(row) = rows
        .next()
        .await
        .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?
    {
        let detail: String = row
            .get(3)
            .map_err(|e| VectorError::Storage(StorageError::Query(e.to_string())))?;
        plan.push_str(&detail);
        plan.push('\n');
    }

    Ok(plan)
}

pub fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn validate_query(query: &[f32], embed_dim: u16) -> Result<(), VectorError> {
    if query.len() != embed_dim as usize {
        return Err(VectorError::DimensionMismatch {
            expected: embed_dim as usize,
            got: query.len(),
        });
    }
    if query.len() > 65_536 {
        return Err(VectorError::DimensionMismatch {
            expected: embed_dim as usize,
            got: query.len(),
        });
    }
    for &x in query {
        if x.is_nan() || x.is_infinite() {
            return Err(VectorError::NonFinite);
        }
    }
    Ok(())
}

fn validate_top_k(top_k: usize) -> Result<(), VectorError> {
    if !(1..=100).contains(&top_k) {
        return Err(VectorError::InvalidTopK(top_k));
    }
    Ok(())
}
