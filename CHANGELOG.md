# Changelog

## v1.0.0 — 2026-06-01

Initial open source release.

### Core
- Document CRUD with content deduplication (SHA-256 hash check)
- Document versioning (previous content saved on update)
- Bulk import up to 1000 documents per batch
- Soft-delete with admin hard-delete via cleanup endpoint
- Per-token rate limiting (60 RPM default, configurable)

### Search
- **Semantic search**: auto-embedding via Workers AI (bge-base-en-v1.5, 768-dim), indexed in Cloudflare Vectorize
- **Hybrid search**: combines vector similarity with knowledge graph traversal, configurable graph_weight and graph_depth
- **Full-text search**: SQLite FTS5 with MATCH syntax, snippets, BM25 ranking
- **Metadata filtering**: filter vector search by string/number/boolean metadata fields

### Knowledge Graph (GraphRAG)
- Auto entity extraction via Workers AI (GLM-4.7-Flash) on document create/update
- 7 entity types: person, organization, concept, location, event, technology, other
- 6 relationship types: related_to, works_at, located_in, part_of, created_by, uses
- Multi-hop bidirectional graph traversal via recursive CTEs
- Async extraction via `ctx.waitUntil()` (response in <1s)

### Sync
- CRDT sync with Hybrid Logical Clock timestamps
- Push/pull protocol for offline-first operation

### Integrations
- MCP server with 12 tools for AI assistants (Claude, etc.)
- TypeScript client SDK
- Python client SDK (zero dependencies)
- OpenAPI 3.1 specification
- Integration guide with curl examples

### Admin
- `init-schema`: create FTS, versioning, rate limit tables + content_hash index
- `backfill-embeddings`: re-embed all documents with metadata
- `backfill-entities`: extract entities for docs without them (50/call)
- `backfill-fts`: rebuild full-text search index
- `cleanup`: purge stale rate limits, hard-delete soft-deleted docs, drop unused tables

### Infrastructure
- Cloudflare Workers + D1 + Vectorize + Workers AI (edge deployment)
- Rust workspace with Axum, libSQL, Redpanda, ClickHouse (local/k8s deployment)
- Docker Compose, Helm chart, federation compose
