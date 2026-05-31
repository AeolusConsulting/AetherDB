use std::sync::Arc;

use aetherdb_embedding_worker::EmbeddingProvider;
use aetherdb_graph::GraphQuerier;
use aetherdb_hnsw::HnswIndex;
use aetherdb_messaging::EventProducer;
use aetherdb_storage::LibsqlStorage;
use aetherdb_sync::{HybridClock, StdClock};
use aetherdb_vector::VectorSearcher;

#[derive(Clone)]
pub struct AppState {
    pub storage: Arc<LibsqlStorage>,
    pub vector_searcher: Option<Arc<VectorSearcher>>,
    pub graph_querier: Option<Arc<GraphQuerier>>,
    pub hnsw_index: Option<Arc<HnswIndex>>,
    pub producer: Option<Arc<EventProducer>>,
    pub embed_provider: Option<Arc<dyn EmbeddingProvider>>,
    pub hlc_clock: Option<Arc<tokio::sync::Mutex<HybridClock<StdClock>>>>,
    pub embed_dim: u16,
}
