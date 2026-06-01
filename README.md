# AetherDB

An AI-native embedded operational database platform built in Rust. Semantic vector search, knowledge graph (GraphRAG), event streaming, CRDT sync, and edge deployment — in a single system.

## What It Does

You store documents. AetherDB automatically:

1. **Embeds them** into vectors via Workers AI / Ollama / OpenAI
2. **Extracts entities and relationships** into a knowledge graph via LLM (GLM-4.7-Flash on edge, Ollama/OpenAI on Rust stack)
3. **Indexes them** for fast semantic search (Vectorize on edge, DiskANN + optional HNSW on Rust stack)
4. **Streams events** to analytics (Redpanda → ClickHouse)
5. **Syncs them** across nodes with conflict resolution (CRDT with Hybrid Logical Clocks)

```bash
# Store a document
curl -X POST http://localhost:8080/v1/documents \
  -H "Content-Type: application/json" \
  -d '{"content": "Rust ownership model prevents data races at compile time."}'

# Semantic search
curl -X POST http://localhost:8080/v1/search/semantic \
  -H "Content-Type: application/json" \
  -d '{"query": "memory safety in programming languages", "top_k": 5}'

# Hybrid search (vector + knowledge graph)
curl -X POST http://localhost:8080/v1/search/hybrid \
  -H "Content-Type: application/json" \
  -d '{"query": "Rust", "graph_depth": 3, "graph_weight": 0.4}'

# Explore the knowledge graph
curl http://localhost:8080/v1/graph/entities/Rust/related?depth=2
```

## Quick Start

### Deploy to Cloudflare Workers (recommended)

```bash
cd cf-worker
npm install
cp wrangler.toml.example wrangler.toml
# Edit wrangler.toml with your account_id and database_id

npx wrangler d1 create aetherdb
npx wrangler vectorize create aetherdb-vectors --dimensions=768 --metric=cosine
npx wrangler secret put API_TOKEN
npx wrangler deploy
```

Then initialize the database tables:

```bash
curl -X POST https://your-worker.workers.dev/v1/admin/init-schema \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN"
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for full setup including migrations and Vectorize metadata indexes.

### Run locally with Docker (Rust stack)

```bash
cp .env.example .env
docker compose up -d
```

## Architecture

```
Client/AI ──→ API Gateway (Axum) ──→ libSQL + DiskANN vectors
                    │                        │
                    ▼                        ▼
              Redpanda events      Embedding Worker (Ollama/OpenAI)
                    │              Graph Worker (LLM entity extraction)
                    ▼              HNSW Index (in-memory, 1M-10M vectors)
              ClickHouse
              (analytics)

Edge:  Cloudflare Workers + D1 + Vectorize + Workers AI  |  WASM in browser
Sync:  HLC clocks + LWW merge + hub-and-spoke federation
Tools: MCP server  |  TypeScript SDK  |  Python SDK
```

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check with doc/entity/relationship counts |
| POST | `/v1/documents` | Create document (auto-embeds, extracts entities, deduplicates) |
| POST | `/v1/documents/bulk` | Bulk import up to 1000 documents |
| GET | `/v1/documents/list` | Paginated list with filters |
| GET | `/v1/documents/:id` | Get document |
| PUT | `/v1/documents/:id` | Update document (saves previous version) |
| DELETE | `/v1/documents/:id` | Delete document (soft-delete) |
| GET | `/v1/documents/:id/versions` | Version history for a document |
| POST | `/v1/search/semantic` | Vector search with optional metadata filtering |
| POST | `/v1/search/hybrid` | Vector + knowledge graph hybrid search |
| POST | `/v1/search/fulltext` | Full-text keyword search (FTS5) |
| POST | `/v1/query` | Read-only SQL query proxy |
| GET | `/v1/graph/entities?document_id=X` | Entities in a document |
| GET | `/v1/graph/entities/:name/related` | Multi-hop graph traversal |
| POST | `/v1/sync/pull` | Pull changes since HLC timestamp |
| POST | `/v1/sync/push` | Push changes for LWW merge |

Full OpenAPI spec: [`docs/openapi.yaml`](docs/openapi.yaml)

## Features

### Data Warehouse
Bulk import documents, browse with pagination and filters, and run arbitrary read-only SQL against the database — including `json_extract()` for querying metadata fields.

```bash
# Bulk import
curl -X POST https://your-aetherdb.workers.dev/v1/documents/bulk \
  -H "Authorization: Bearer $TOKEN" \
  -d '[{"content": "...", "metadata": {"category": "ai"}}]'

# List with filters
curl "https://your-aetherdb.workers.dev/v1/documents/list?limit=50&content_contains=rust"

# SQL query (read-only)
curl -X POST https://your-aetherdb.workers.dev/v1/query \
  -d '{"sql": "SELECT json_extract(metadata, '$.category') as cat, COUNT(*) as n FROM documents WHERE deleted=0 GROUP BY cat"}'
```

### Semantic Vector Search
Documents are automatically embedded on create, update, and bulk import. The edge deployment (Cloudflare Workers) uses Workers AI (`@cf/baai/bge-base-en-v1.5`, 768-dim) with Cloudflare Vectorize for indexed ANN search — clients just send text queries, no embeddings needed. The Rust stack uses Ollama or OpenAI for embeddings with libSQL's DiskANN index and optional in-memory HNSW for 1M-10M vector scale.

```bash
# Search — just send text, server handles embedding
curl -X POST https://your-aetherdb.workers.dev/v1/search/semantic \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "memory safety in programming", "top_k": 5}'

# Search with metadata filter
curl -X POST https://your-aetherdb.workers.dev/v1/search/semantic \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "databases", "top_k": 5, "filter": {"source": "myapp"}}'
```

### Full-Text Search
SQLite FTS5 for exact keyword matching — complements semantic search when you need precise term hits. Supports AND, OR, NOT operators, phrase matching, and returns highlighted snippets with BM25 ranking.

```bash
curl -X POST https://your-aetherdb.workers.dev/v1/search/fulltext \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "Rust ownership", "top_k": 10}'
```

### Document Versioning
Every update saves the previous version. Retrieve the full history for any document to see how it changed over time.

```bash
curl "https://your-aetherdb.workers.dev/v1/documents/YOUR-UUID/versions" \
  -H "Authorization: Bearer $TOKEN"
```

### GraphRAG
An LLM automatically extracts entities (person, organization, concept, location, event, technology) and relationships from every document on create and update. The edge deployment uses Workers AI GLM-4.7-Flash; the Rust stack uses Ollama or OpenAI. The knowledge graph enables multi-hop bidirectional traversal via recursive CTEs — find documents connected through shared entities across relationship chains.

```bash
# Hybrid search — combines vector similarity with graph traversal
curl -X POST https://your-aetherdb.workers.dev/v1/search/hybrid \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "Rust memory safety", "top_k": 5, "graph_depth": 2, "graph_weight": 0.3}'

# Explore the knowledge graph
curl "https://your-aetherdb.workers.dev/v1/graph/entities?document_id=YOUR-UUID" \
  -H "Authorization: Bearer $TOKEN"

# Multi-hop graph traversal
curl "https://your-aetherdb.workers.dev/v1/graph/entities/Rust/related?depth=3" \
  -H "Authorization: Bearer $TOKEN"
```

Hybrid search scoring: `final = (1 - graph_weight) * vector_score + graph_weight * graph_score`, where graph score is `1 / (1 + hop_count)`. Documents matching both vector similarity and graph context rank highest.

### CRDT Sync
Every document carries a Hybrid Logical Clock timestamp. Conflicts resolve deterministically via Last-Writer-Wins with NodeId tiebreaker. Delta sync protocol (push/pull) enables offline-first operation — nodes diverge and converge without manual conflict resolution.

### Federated Replication
Hub-and-spoke topology: spoke nodes periodically push local changes to the hub and pull hub changes. All nodes converge to the same state. Configure with `AETHERDB_FEDERATION_ENABLED=true`.

### Event Streaming
Every write emits events to Redpanda (Kafka-compatible). The analytics consumer batches events into ClickHouse for OLAP queries. The DuckDB exporter produces periodic Parquet snapshots.

### Edge Deployment
- **Cloudflare Workers + D1 + Vectorize**: Full API at the edge with SQLite storage, auto-embedding via Workers AI, indexed vector search via Vectorize, and LLM-powered entity extraction via GLM-4.7-Flash
- **WASM crate**: In-memory store with brute-force cosine search, runs in browsers
- **Embedded replicas**: libSQL replicas sync from sqld primary

## Project Structure

```
apps/
  api-gateway/          Axum HTTP server (all API endpoints)
  embedding-worker/     Consumes events, generates embeddings
  graph-worker/         Consumes events, extracts entities via LLM
  analytics-consumer/   Redpanda → ClickHouse pipeline
  duckdb-exporter/      Periodic Parquet exports

crates/
  domain/               Types, traits, errors (zero infra deps)
  storage/              libSQL adapter, repos, migrations
  vector/               vector_top_k search, score conversion
  graph/                Recursive CTE graph traversal
  hnsw/                 In-memory HNSW (instant-distance)
  events/               Event types and Envelope
  messaging/            Redpanda producer/consumer (rdkafka)
  sync/                 HLC, LWW merge, delta sync protocol
  edge/                 WASM-compatible in-memory store
  config/               Env parsing, validation
  telemetry/            Tracing, Prometheus, OTLP

cf-worker/              Cloudflare Workers deployment (D1 + Vectorize + Workers AI)
mcp-server/             MCP server for AI assistants (8 tools)
sdks/
  typescript/           TypeScript client SDK
  python/               Python client SDK (zero deps)
helm/aetherdb/          Kubernetes Helm chart (23 resources)
```

## Deployment Options

| Mode | Command | Best For |
|------|---------|----------|
| **Local** | `docker compose up` | Development |
| **Distributed** | `docker compose -f docker-compose.yml -f docker-compose.distributed.yml up` | Read-scaling with replicas |
| **Federated** | `docker compose -f docker-compose.yml -f docker-compose.federation.yml up` | Multi-region sync |
| **Kubernetes** | `helm install aetherdb ./helm/aetherdb/` | Production clusters |
| **Edge** | `cd cf-worker && npx wrangler deploy` | Serverless/edge |
| **Browser** | `wasm-pack build crates/edge --target web --features wasm` | Client-side search |

## Client SDKs

### TypeScript
```typescript
import { AetherDB } from '@aetherdb/client';
const db = new AetherDB('http://localhost:8080', 'api-key');
const doc = await db.createDocument('Hello world', { source: 'test' });
const results = await db.search('Hello');
```

### Python
```python
from aetherdb import AetherDB
db = AetherDB('http://localhost:8080', api_key='api-key')
doc = db.create_document('Hello world', metadata={'source': 'test'})
results = db.search('Hello')
```

### MCP (AI Assistants)
Add to Claude Desktop config:
```json
{
  "mcpServers": {
    "aetherdb": {
      "command": "node",
      "args": ["mcp-server/src/index.js"],
      "env": { "AETHERDB_GATEWAY_URL": "http://localhost:8080" }
    }
  }
}
```

Tools: `aetherdb_create_document`, `aetherdb_search`, `aetherdb_hybrid_search`, `aetherdb_get_entities`, `aetherdb_get_related`, and more.

## Configuration

All via `AETHERDB_*` environment variables. Key settings:

| Variable | Default | Description |
|----------|---------|-------------|
| `AETHERDB_LIBSQL_URL` | — | Database URL (`file:./aether.db` or `http://sqld:8080`) |
| `AETHERDB_REDPANDA_BROKERS` | — | Kafka brokers |
| `AETHERDB_EMBED_PROVIDER` | `ollama` | `ollama` or `openai` |
| `AETHERDB_GRAPH_ENABLED` | `false` | Enable GraphRAG entity extraction |
| `AETHERDB_HNSW_ENABLED` | `false` | Enable in-memory HNSW index |
| `AETHERDB_FEDERATION_ENABLED` | `false` | Enable multi-region sync |
| `AETHERDB_API_KEYS` | (empty) | API keys for auth (empty = disabled) |

Full reference: [`.env.example`](.env.example) | [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md)

## Documentation

- [OpenAPI Spec](docs/openapi.yaml) — full API reference
- [Architecture Guide](docs/ARCHITECTURE.md) — system design, data flow, CRDT protocol
- [Deployment Guide](docs/DEPLOYMENT.md) — Docker, Helm, federation, monitoring

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust 2024 edition (1.85+) |
| HTTP Server | Axum 0.7 |
| Database | libSQL (embedded SQLite with vector extensions) |
| Vector Index | DiskANN (native) + HNSW (instant-distance) |
| Event Streaming | Redpanda (Kafka-compatible) |
| Analytics | ClickHouse |
| Export | DuckDB → Parquet |
| Embeddings | Workers AI (bge-base-en-v1.5) / Ollama / OpenAI |
| LLM (GraphRAG) | Workers AI (GLM-4.7-Flash) / Ollama / OpenAI |
| Edge | Cloudflare Workers + D1 + Vectorize + Workers AI |
| WASM | wasm32-unknown-unknown + wasm32-wasi |
| Orchestration | Kubernetes (Helm) / Docker Compose |
| Monitoring | Prometheus + Grafana |

## License

Apache-2.0
