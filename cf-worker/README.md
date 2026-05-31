# AetherDB Cloudflare Worker

AetherDB running on Cloudflare Workers with D1 (SQLite) for persistent storage.

## Features

- Document CRUD (create, read, update, soft-delete)
- Brute-force cosine similarity search (no vector index — D1 limitation)
- Entity/relationship graph queries
- CRDT sync endpoints (push/pull with HLC timestamps)

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| POST | `/v1/documents` | Create document |
| GET | `/v1/documents/:id` | Get document |
| PUT | `/v1/documents/:id` | Update document |
| DELETE | `/v1/documents/:id` | Soft-delete document |
| POST | `/v1/search/semantic` | Vector search (requires `query_embedding` array) |
| GET | `/v1/graph/entities?document_id=X` | Get entities |
| GET | `/v1/graph/entities/:name/related?depth=2` | Related documents |
| POST | `/v1/sync/pull` | Pull changes since HLC |
| POST | `/v1/sync/push` | Push changes for merge |

## Deploy

```bash
cd cf-worker
npm install
npx wrangler deploy
```

## Local Development

```bash
npx wrangler dev
```

## Notes

- D1 does not support libSQL vector extensions, so search uses brute-force cosine distance
- For vector search, pass `query_embedding` (float array) in the search request body
- Embeddings stored as JSON arrays in the `embeddings` table
- Soft-delete with tombstones for CRDT sync compatibility
