# aetherdb-python

Python client SDK for AetherDB. Zero dependencies (uses stdlib `urllib`).

```python
from aetherdb import AetherDB

db = AetherDB('https://your-aetherdb.workers.dev', api_key='your-key')

# Create a document (auto-embeds, extracts entities, deduplicates)
doc = db.create_document('Hello world', metadata={'source': 'test'})

# Semantic search
results = db.search('Hello')
for hit in results:
    print(f"{hit.document_id}: {hit.score:.3f} - {hit.content_preview}")

# Search with metadata filter
filtered = db.search('Hello', filter={'source': 'test'})

# Hybrid search (vector + knowledge graph)
results = db.hybrid_search('machine learning', graph_depth=3, graph_weight=0.3)

# Full-text keyword search
results = db.fulltext_search('exact phrase')
for hit in results:
    print(f"{hit.document_id}: {hit.snippet}")

# List documents with filters
docs = db.list_documents(limit=20, content_contains='rust')

# Get version history
versions = db.get_versions(doc['id'])
for v in versions:
    print(f"{v.created_at}: {v.content[:80]}")

# SQL query
result = db.query('SELECT COUNT(*) as n FROM documents WHERE deleted=0')

# Bulk import with entity extraction
result = db.bulk_import(
    [{'content': 'Doc 1'}, {'content': 'Doc 2'}],
    extract_entities=True
)

# Knowledge graph
entities = db.get_entities(doc['id'])
for e in entities:
    print(f"{e.name} ({e.entity_type})")

related = db.get_related_documents('Rust', depth=2)

# Health check
health = db.health()
# {'status': 'ok', 'documents': 14, 'entities': 90, 'relationships': 58}
```

## Methods

| Method | Description |
|--------|-------------|
| `create_document(content, metadata?, deduplicate?)` | Create document |
| `get_document(doc_id)` | Get document by UUID |
| `update_document(doc_id, content, metadata?)` | Update (saves previous version) |
| `delete_document(doc_id)` | Soft-delete |
| `get_versions(doc_id)` | Get version history |
| `list_documents(limit, offset, content_contains, ...)` | List with filters |
| `bulk_import(documents, extract_entities?)` | Bulk import |
| `search(query, top_k, filter?, return_content?)` | Semantic vector search |
| `hybrid_search(query, top_k, graph_depth, graph_weight, filter?, ...)` | Hybrid search |
| `fulltext_search(query, top_k, return_content?)` | FTS5 keyword search |
| `query(sql)` | Read-only SQL query |
| `get_entities(document_id)` | Get extracted entities |
| `get_related_documents(entity_name, depth?)` | Multi-hop graph traversal |
| `sync_pull(since)` | Pull changes since HLC timestamp |
| `sync_push(changes)` | Push changes for CRDT merge |
| `health()` | Health check with counts |
