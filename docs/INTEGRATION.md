# AetherDB Integration Guide

## Base URL

```
https://your-aetherdb.workers.dev
```

## Authentication

All endpoints except `/health` require a Bearer token:

```
Authorization: Bearer YOUR_API_TOKEN
```

Admin endpoints (`/v1/admin/*`) require a separate admin token.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check with doc/entity/relationship counts |
| POST | `/v1/documents` | Create document (auto-embeds, extracts entities, deduplicates) |
| POST | `/v1/documents/bulk` | Batch import (max 1000) |
| GET | `/v1/documents/list` | Paginated list with filters |
| GET | `/v1/documents/:id` | Get document |
| PUT | `/v1/documents/:id` | Update document (saves previous version) |
| DELETE | `/v1/documents/:id` | Soft-delete |
| GET | `/v1/documents/:id/versions` | Version history |
| POST | `/v1/search/semantic` | Vector search with optional metadata filtering |
| POST | `/v1/search/hybrid` | Vector + knowledge graph hybrid search |
| POST | `/v1/search/fulltext` | Full-text keyword search (FTS5) |
| POST | `/v1/query` | Read-only SQL (SELECT/WITH only) |
| GET | `/v1/graph/entities?document_id=X` | Entities in a document |
| GET | `/v1/graph/entities/:name/related?depth=N` | Multi-hop graph traversal |
| POST | `/v1/sync/push` | Push changes for CRDT merge |
| POST | `/v1/sync/pull` | Pull changes since HLC timestamp |

## Quick Examples

### Store a document

```bash
curl -X POST https://your-aetherdb.workers.dev/v1/documents \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "Your text here", "metadata": {"source": "myapp", "category": "notes"}}'
```

Response (returns in <1s, entities extracted in background):
```json
{"id": "uuid", "created_at": "2026-...", "embedded": true, "entities": "processing"}
```

Duplicate content returns the existing document:
```json
{"id": "existing-uuid", "created_at": "2026-...", "duplicate": true}
```

Opt out of deduplication with `"deduplicate": false`.

### Bulk import

```bash
# Simple array format
curl -X POST https://your-aetherdb.workers.dev/v1/documents/bulk \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '[
    {"content": "First document", "metadata": {"category": "test"}},
    {"content": "Second document", "metadata": {"category": "test"}}
  ]'

# With entity extraction (runs in background)
curl -X POST https://your-aetherdb.workers.dev/v1/documents/bulk \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "documents": [{"content": "...", "metadata": {}}],
    "extract_entities": true
  }'
```

### List documents

```bash
curl "https://your-aetherdb.workers.dev/v1/documents/list?limit=10&content_contains=rust" \
  -H "Authorization: Bearer $TOKEN"
```

Filters: `limit`, `offset`, `content_contains`, `created_after` (unix ms), `created_before` (unix ms).

### Update a document

```bash
curl -X PUT https://your-aetherdb.workers.dev/v1/documents/YOUR-UUID \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"content": "Updated text", "metadata": {"version": 2}}'
```

Previous content is saved automatically. Retrieve version history:

```bash
curl "https://your-aetherdb.workers.dev/v1/documents/YOUR-UUID/versions" \
  -H "Authorization: Bearer $TOKEN"
```

### Semantic vector search

```bash
curl -X POST https://your-aetherdb.workers.dev/v1/search/semantic \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "memory safety in programming", "top_k": 5}'
```

With metadata filter:
```bash
curl -X POST https://your-aetherdb.workers.dev/v1/search/semantic \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "databases", "top_k": 5, "filter": {"source": "myapp"}}'
```

With full content in results:
```bash
curl -X POST https://your-aetherdb.workers.dev/v1/search/semantic \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "databases", "top_k": 5, "return_content": true}'
```

### Hybrid search (vector + knowledge graph)

Combines vector similarity with graph traversal. Documents matching both rank highest.

```bash
curl -X POST https://your-aetherdb.workers.dev/v1/search/hybrid \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Rust memory safety",
    "top_k": 5,
    "graph_depth": 2,
    "graph_weight": 0.3
  }'
```

Scoring: `final = (1 - graph_weight) * vector_score + graph_weight * graph_score`

Parameters:
- `graph_depth` (0-5, default 2): how many hops to traverse in the entity graph
- `graph_weight` (0-1, default 0.3): weight of graph vs vector score
- `filter`: optional metadata filter (same as semantic search)
- `return_content`: include full document content

### Full-text keyword search

SQLite FTS5 for exact keyword matching. Supports AND, OR, NOT, phrase matching.

```bash
curl -X POST https://your-aetherdb.workers.dev/v1/search/fulltext \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "Rust ownership", "top_k": 10}'
```

Response includes highlighted snippets and BM25 ranking:
```json
{
  "results": [
    {"document_id": "...", "snippet": "<b>Rust</b> <b>ownership</b> model prevents...", "rank": -3.887}
  ],
  "count": 1
}
```

### SQL query

```bash
curl -X POST https://your-aetherdb.workers.dev/v1/query \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"sql": "SELECT id, json_extract(metadata, '$.source') as src, substr(content, 1, 100) as preview FROM documents WHERE deleted=0 ORDER BY created_at DESC LIMIT 20"}'
```

Only SELECT/WITH queries allowed. Write operations, ATTACH, PRAGMA are blocked.

### Knowledge graph

```bash
# Get entities extracted from a document
curl "https://your-aetherdb.workers.dev/v1/graph/entities?document_id=YOUR-UUID" \
  -H "Authorization: Bearer $TOKEN"

# Find related documents via graph traversal
curl "https://your-aetherdb.workers.dev/v1/graph/entities/Rust/related?depth=2" \
  -H "Authorization: Bearer $TOKEN"
```

Entity types: `person`, `organization`, `concept`, `location`, `event`, `technology`, `other`
Relationship types: `related_to`, `works_at`, `located_in`, `part_of`, `created_by`, `uses`

## Client SDK Examples

### TypeScript / JavaScript

```typescript
const BASE = 'https://your-aetherdb.workers.dev';
const TOKEN = 'YOUR_TOKEN';

const headers = {
  'Authorization': `Bearer ${TOKEN}`,
  'Content-Type': 'application/json'
};

// Create
const doc = await fetch(`${BASE}/v1/documents`, {
  method: 'POST', headers,
  body: JSON.stringify({ content: 'Hello world', metadata: { source: 'myapp' } })
}).then(r => r.json());

// Semantic search with filter
const results = await fetch(`${BASE}/v1/search/semantic`, {
  method: 'POST', headers,
  body: JSON.stringify({ query: 'hello', top_k: 5, filter: { source: 'myapp' } })
}).then(r => r.json());

// Hybrid search
const hybrid = await fetch(`${BASE}/v1/search/hybrid`, {
  method: 'POST', headers,
  body: JSON.stringify({ query: 'hello', graph_depth: 2, graph_weight: 0.3 })
}).then(r => r.json());

// Full-text search
const fts = await fetch(`${BASE}/v1/search/fulltext`, {
  method: 'POST', headers,
  body: JSON.stringify({ query: 'hello world' })
}).then(r => r.json());

// Version history
const versions = await fetch(`${BASE}/v1/documents/${doc.id}/versions`, { headers })
  .then(r => r.json());
```

### Python

```python
import requests

BASE = 'https://your-aetherdb.workers.dev'
TOKEN = 'YOUR_TOKEN'
headers = {'Authorization': f'Bearer {TOKEN}', 'Content-Type': 'application/json'}

# Create
doc = requests.post(f'{BASE}/v1/documents',
    json={'content': 'Hello world', 'metadata': {'source': 'myapp'}},
    headers=headers).json()

# Semantic search with filter
results = requests.post(f'{BASE}/v1/search/semantic',
    json={'query': 'hello', 'top_k': 5, 'filter': {'source': 'myapp'}},
    headers=headers).json()

# Hybrid search
hybrid = requests.post(f'{BASE}/v1/search/hybrid',
    json={'query': 'hello', 'graph_depth': 2, 'graph_weight': 0.3},
    headers=headers).json()

# Full-text search
fts = requests.post(f'{BASE}/v1/search/fulltext',
    json={'query': 'hello world'},
    headers=headers).json()

# Version history
versions = requests.get(f'{BASE}/v1/documents/{doc["id"]}/versions',
    headers=headers).json()
```

## Notes

- All responses are JSON with `Access-Control-Allow-Origin: *`
- Content max size: 1MB per document
- Bulk import max: 1000 documents per request
- Deduplication: on by default (content_hash check), opt-out with `deduplicate: false`
- Entity extraction: async on create/update (~2-5s in background), opt-in for bulk import
- Document versioning: previous content saved automatically on update
- SQL endpoint: SELECT/WITH only (write operations blocked)
- Metadata: freeform JSON, queryable via `json_extract()` in SQL, filterable in vector search
- Metadata filtering: string, number, boolean fields are indexed in Vectorize (source, category, created_at)
- Soft-delete: deleted docs are hidden from GET/list but preserved for sync
- Timestamps: ISO 8601 in responses, Unix milliseconds internally

## Database Schema

```sql
-- Documents
SELECT id, content, metadata, content_hash, created_at, updated_at FROM documents WHERE deleted = 0;

-- Document versions (saved on update)
SELECT id, document_id, content, metadata, content_hash, created_at FROM document_versions;

-- Entities (auto-extracted by LLM)
SELECT id, name, entity_type, document_id FROM entities;

-- Relationships
SELECT source_entity_id, target_entity_id, relationship_type, weight FROM relationships;

-- Full-text search (FTS5)
SELECT id, content FROM documents_fts WHERE documents_fts MATCH 'query';
```

## Useful SQL Queries

```sql
-- Count documents by metadata field
SELECT json_extract(metadata, '$.source') as src, COUNT(*) as n
FROM documents WHERE deleted=0 GROUP BY src

-- Recent documents
SELECT id, substr(content, 1, 100) as preview, created_at
FROM documents WHERE deleted=0 ORDER BY created_at DESC LIMIT 10

-- Entity frequency
SELECT name, entity_type, COUNT(*) as mentions
FROM entities GROUP BY name ORDER BY mentions DESC LIMIT 20

-- Relationship map
SELECT e1.name as source, r.relationship_type, e2.name as target
FROM relationships r
JOIN entities e1 ON e1.id = r.source_entity_id
JOIN entities e2 ON e2.id = r.target_entity_id
LIMIT 50
```

## Admin Endpoints

Require `ADMIN_TOKEN` (set via `npx wrangler secret put ADMIN_TOKEN`).

| Endpoint | Description |
|----------|-------------|
| `POST /v1/admin/init-schema` | Create FTS + versions tables + content_hash index |
| `POST /v1/admin/backfill-embeddings` | Re-embed all documents (with metadata) |
| `POST /v1/admin/backfill-entities` | Extract entities for docs without them (50/call) |
| `POST /v1/admin/backfill-fts` | Rebuild full-text search index |
