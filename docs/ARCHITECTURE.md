# AetherDB Architecture

## System Overview

```
┌──────────────────────────────────────────────────────┐
│                    API Gateway (Axum)                  │
│  /v1/documents  /v1/search  /v1/graph  /v1/sync      │
│  Auth middleware → Rate limit → Request ID → Routes   │
└─────────────┬────────────────────────────┬───────────┘
              │                            │
              ▼                            ▼
┌─────────────────────┐    ┌──────────────────────────┐
│ libSQL (embedded)    │    │ Redpanda (events)        │
│ + sqld (distributed) │    │ documents.created/updated │
│ + DiskANN vectors    │    │ embeddings.created        │
└─────────────────────┘    │ entities.extracted         │
                           └──────┬───────────┬────────┘
                                  ▼           ▼
                    ┌──────────────┐ ┌────────────────┐
                    │ Embedding    │ │ Graph Worker    │
                    │ Worker       │ │ (LLM entity    │
                    │ (Ollama/     │ │  extraction)   │
                    │  OpenAI)     │ └────────────────┘
                    └──────────────┘
                           ▼
              ┌────────────────────────┐
              │ Analytics Consumer     │
              │ Redpanda → ClickHouse  │
              └────────────────────────┘
```

## Crate Dependency Graph

```
domain (types, traits, errors — zero infra deps)
  ├── events (event types, Envelope)
  ├── sync (HLC, LWW merge, delta protocol)
  ├── storage (libSQL adapter, repos, migrations)
  │     ├── vector (vector_top_k search)
  │     ├── graph (recursive CTE traversal)
  │     └── hnsw (in-memory instant-distance)
  ├── config (env parsing, validation)
  ├── telemetry (tracing, Prometheus, OTLP)
  ├── messaging (rdkafka producer/consumer)
  └── edge (WASM, in-memory store, brute-force search)
```

## Data Flow

### Document Ingestion
1. Client → `POST /v1/documents` → API Gateway
2. Gateway validates, computes SHA-256 hash, inserts into libSQL
3. Gateway publishes `DocumentCreated` event to Redpanda
4. Embedding Worker consumes event → fetches content → calls Ollama/OpenAI → stores F32_BLOB embedding
5. Graph Worker consumes event → fetches content → calls LLM → extracts entities/relationships
6. Analytics Consumer consumes all events → batch inserts to ClickHouse

### Semantic Search
1. Client → `POST /v1/search/semantic` → API Gateway
2. Gateway embeds query text via Ollama/OpenAI provider
3. Gateway calls `vector_top_k` on libSQL's DiskANN index (or HNSW if enabled)
4. Returns ranked results with cosine similarity scores

### Hybrid Search
1. Same as semantic search, plus:
2. Graph traversal via recursive CTE finds related documents through entity/relationship graph
3. Scores merged: `(1 - weight) * vector_score + weight * graph_score`

## CRDT Sync Protocol

### Hybrid Logical Clock (HLC)
- `HlcTimestamp { wall_ms: i64, counter: u32 }` — total ordering
- `HybridClock::now()` — advances clock for local events
- `HybridClock::recv(remote)` — merges with incoming timestamp

### LWW-Register Merge
- Per-document Last-Writer-Wins based on HLC timestamp
- Tiebreaker: higher NodeId (UUID comparison) wins
- Tombstones (deleted=true) participate in LWW — higher HLC decides

### Federation (Hub-and-Spoke)
- Spokes periodically push local changes to hub
- Spokes pull hub changes and apply LWW merge
- Hub is a regular gateway instance with `FEDERATION_ROLE=hub`
- Delta sync: changes since last HLC cursor

## Storage Layers

| Layer | Technology | Purpose |
|-------|-----------|---------|
| Operational DB | libSQL (embedded) | Document CRUD, embeddings, entities |
| Vector Index | libsql_vector_idx (DiskANN) | Approximate nearest neighbor |
| HNSW Index | instant-distance (in-memory) | Optional faster ANN for 1M+ vectors |
| Event Backbone | Redpanda | Async event streaming |
| Analytics | ClickHouse | OLAP queries on events |
| Export | DuckDB → Parquet | Periodic snapshots |
| Edge | In-memory HashMap | WASM-compatible local store |

## Configuration

All via `AETHERDB_*` environment variables. See `.env.example` for the full list.
Key feature flags: `AETHERDB_MINIMAL_MODE`, `AETHERDB_GRAPH_ENABLED`, `AETHERDB_HNSW_ENABLED`, `AETHERDB_FEDERATION_ENABLED`.
