# AetherDB MCP Server

MCP (Model Context Protocol) server that exposes AetherDB as tools for AI assistants like Claude.

## Tools

| Tool | Description |
|------|-------------|
| `aetherdb_create_document` | Store a document (auto-embedded, entities extracted in background, deduplicates) |
| `aetherdb_get_document` | Retrieve a document by UUID |
| `aetherdb_update_document` | Update document content and metadata (saves previous version) |
| `aetherdb_delete_document` | Delete a document |
| `aetherdb_search` | Semantic vector search with optional metadata filtering |
| `aetherdb_hybrid_search` | Combined vector + knowledge graph search with configurable weights |
| `aetherdb_fulltext_search` | Full-text keyword search (FTS5 MATCH syntax, snippets, BM25 ranking) |
| `aetherdb_list_documents` | List documents with pagination and filters |
| `aetherdb_sql_query` | Execute read-only SQL queries against the database |
| `aetherdb_get_versions` | Get version history for a document |
| `aetherdb_get_entities` | Get extracted entities from a document |
| `aetherdb_get_related` | Find related documents via multi-hop graph traversal |

## Setup

```bash
cd mcp-server
npm install
```

## Configure in Claude Desktop

Add to `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "aetherdb": {
      "command": "node",
      "args": ["/path/to/Aetherdb/mcp-server/src/index.js"],
      "env": {
        "AETHERDB_GATEWAY_URL": "https://your-aetherdb.workers.dev",
        "AETHERDB_API_KEY": "your-api-key"
      }
    }
  }
}
```

## Configure in Claude Code

Add to `.claude/settings.json`:

```json
{
  "mcpServers": {
    "aetherdb": {
      "command": "node",
      "args": ["/path/to/Aetherdb/mcp-server/src/index.js"],
      "env": {
        "AETHERDB_GATEWAY_URL": "https://your-aetherdb.workers.dev",
        "AETHERDB_API_KEY": "your-api-key"
      }
    }
  }
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AETHERDB_GATEWAY_URL` | `http://localhost:8080` | AetherDB API URL |
| `AETHERDB_API_KEY` | (empty) | API key for authentication |
