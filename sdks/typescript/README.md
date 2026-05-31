# @aetherdb/client

TypeScript client SDK for AetherDB.

```typescript
import { AetherDB } from './aetherdb';

const db = new AetherDB('https://your-aetherdb.workers.dev', 'your-api-key');

// Create a document (auto-embeds, extracts entities, deduplicates)
const doc = await db.createDocument('Hello world', { source: 'test' });

// Semantic search
const results = await db.search('Hello');

// Search with metadata filter
const filtered = await db.search('Hello', { filter: { source: 'test' } });

// Hybrid search (vector + knowledge graph)
const hybrid = await db.hybridSearch('machine learning', { graphDepth: 3, graphWeight: 0.3 });

// Full-text keyword search
const fts = await db.fulltextSearch('exact phrase');

// List documents with filters
const list = await db.listDocuments({ limit: 20, contentContains: 'rust' });

// Get version history
const versions = await db.getVersions(doc.id);

// SQL query
const rows = await db.query('SELECT COUNT(*) as n FROM documents WHERE deleted=0');

// Bulk import with entity extraction
const bulk = await db.bulkImport(
  [{ content: 'Doc 1' }, { content: 'Doc 2' }],
  { extractEntities: true }
);

// Knowledge graph
const entities = await db.getEntities(doc.id);
const related = await db.getRelatedDocuments('Rust', 2);

// Health check
const health = await db.health();
// { status: 'ok', documents: 14, entities: 90, relationships: 58 }
```

## Methods

| Method | Description |
|--------|-------------|
| `createDocument(content, metadata?, opts?)` | Create document (opts: `{ deduplicate?: boolean }`) |
| `getDocument(id)` | Get document by UUID |
| `updateDocument(id, content, metadata?)` | Update document (saves previous version) |
| `deleteDocument(id)` | Soft-delete document |
| `getVersions(id)` | Get version history |
| `listDocuments(opts?)` | List with `limit`, `offset`, `contentContains`, `createdAfter`, `createdBefore` |
| `bulkImport(docs, opts?)` | Bulk import (opts: `{ extractEntities?: boolean }`) |
| `search(query, opts?)` | Semantic search (opts: `filter`, `topK`, `returnContent`) |
| `hybridSearch(query, opts?)` | Hybrid search (opts: `graphDepth`, `graphWeight`, `filter`, `returnContent`) |
| `fulltextSearch(query, opts?)` | FTS5 keyword search (opts: `topK`, `returnContent`) |
| `query(sql)` | Read-only SQL query |
| `getEntities(documentId)` | Get extracted entities |
| `getRelatedDocuments(name, depth?)` | Multi-hop graph traversal |
| `syncPull(since)` | Pull changes since HLC timestamp |
| `syncPush(changes)` | Push changes for CRDT merge |
| `health()` | Health check with counts |
