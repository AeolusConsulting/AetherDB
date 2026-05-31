use aetherdb_domain::{DocumentId, HnswError, StorageError};
use aetherdb_storage::LibsqlStorage;
use instant_distance::{Builder, HnswMap, Search};
use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct HnswConfig {
    pub ef_construction: usize,
    pub m: usize,
    pub ef_search: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        HnswConfig {
            ef_construction: 100,
            m: 16,
            ef_search: 50,
        }
    }
}

pub struct HnswIndex {
    inner: RwLock<Option<IndexState>>,
    config: HnswConfig,
    embed_dim: u16,
}

struct IndexState {
    map: HnswMap<Point, DocumentId>,
}

#[derive(Clone)]
struct Point(Vec<f32>);

impl instant_distance::Point for Point {
    fn distance(&self, other: &Self) -> f32 {
        let dot: f32 = self.0.iter().zip(other.0.iter()).map(|(a, b)| a * b).sum();
        let norm_a: f32 = self.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = other.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        let denom = norm_a * norm_b;
        if denom == 0.0 {
            1.0
        } else {
            1.0 - (dot / denom)
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub document_id: DocumentId,
    pub score: f32,
    pub distance: f32,
}

impl HnswIndex {
    pub fn new(config: HnswConfig, embed_dim: u16) -> Self {
        HnswIndex {
            inner: RwLock::new(None),
            config,
            embed_dim,
        }
    }

    pub async fn build_from_storage(&self, storage: &LibsqlStorage) -> Result<(), HnswError> {
        let mut rows = storage
            .conn()
            .query("SELECT document_id, embedding FROM document_embeddings", ())
            .await
            .map_err(|e| HnswError::Storage(StorageError::Query(e.to_string())))?;

        let mut points = Vec::new();
        let mut doc_ids = Vec::new();

        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| HnswError::Storage(StorageError::Query(e.to_string())))?
        {
            let doc_id_str: String = row
                .get(0)
                .map_err(|e| HnswError::Storage(StorageError::Query(e.to_string())))?;
            let blob: Vec<u8> = row
                .get(1)
                .map_err(|e| HnswError::Storage(StorageError::Query(e.to_string())))?;

            let uid =
                uuid::Uuid::parse_str(&doc_id_str).map_err(|e| HnswError::Build(e.to_string()))?;

            let floats = bytes_to_f32_vec(&blob);
            if floats.len() == self.embed_dim as usize {
                points.push(Point(floats));
                doc_ids.push(DocumentId(uid));
            }
        }

        if points.is_empty() {
            let mut state = self.inner.write();
            *state = None;
            return Ok(());
        }

        let ef = self.config.ef_construction;
        let map = tokio::task::spawn_blocking(move || {
            Builder::default()
                .ef_construction(ef)
                .build(points, doc_ids)
        })
        .await
        .map_err(|e| HnswError::Build(e.to_string()))?;

        let mut state = self.inner.write();
        *state = Some(IndexState { map });

        tracing::info!(
            "HNSW index built with {} vectors, ef_construction={}",
            state.as_ref().map(|s| s.map.values.len()).unwrap_or(0),
            self.config.ef_construction
        );

        Ok(())
    }

    pub fn search(&self, query: &[f32], top_k: usize) -> Result<Vec<SearchResult>, HnswError> {
        if query.len() != self.embed_dim as usize {
            return Err(HnswError::DimensionMismatch {
                expected: self.embed_dim as usize,
                got: query.len(),
            });
        }

        let guard = self.inner.read();
        let state = guard.as_ref().ok_or(HnswError::NotReady)?;

        let point = Point(query.to_vec());
        let mut search = Search::default();
        let neighbors = state.map.search(&point, &mut search);

        let mut results = Vec::new();
        for item in neighbors.take(top_k) {
            let distance = item.distance;
            let score = (1.0_f32 - distance).clamp(0.0, 1.0);
            results.push(SearchResult {
                document_id: *item.value,
                score,
                distance,
            });
        }

        Ok(results)
    }

    pub fn is_ready(&self) -> bool {
        self.inner.read().is_some()
    }

    pub fn vector_count(&self) -> usize {
        self.inner
            .read()
            .as_ref()
            .map(|s| s.map.values.len())
            .unwrap_or(0)
    }
}

fn bytes_to_f32_vec(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| {
            let arr: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
            f32::from_le_bytes(arr)
        })
        .collect()
}
