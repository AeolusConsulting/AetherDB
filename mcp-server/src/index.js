#!/usr/bin/env node

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";

const GATEWAY_URL = process.env.AETHERDB_GATEWAY_URL || "http://localhost:8080";
const API_KEY = process.env.AETHERDB_API_KEY || "";

function headers() {
  const h = { "Content-Type": "application/json" };
  if (API_KEY) h["Authorization"] = `Bearer ${API_KEY}`;
  return h;
}

async function apiCall(method, path, body) {
  const opts = { method, headers: headers() };
  if (body) opts.body = JSON.stringify(body);
  const resp = await fetch(`${GATEWAY_URL}${path}`, opts);
  if (resp.status === 204) return { status: "ok" };
  const data = await resp.json();
  if (!resp.ok) throw new Error(data.error || `HTTP ${resp.status}`);
  return data;
}

const server = new McpServer({
  name: "aetherdb-mcp",
  version: "0.1.0",
});

// --- Document tools ---

server.tool(
  "aetherdb_create_document",
  "Store a document in AetherDB. It will be automatically embedded for vector search and entities extracted for GraphRAG.",
  {
    content: { type: "string", description: "Document content (text, max 1MB)" },
    metadata: { type: "string", description: "Optional JSON metadata object (default: '{}')" },
  },
  async ({ content, metadata }) => {
    const meta = metadata ? JSON.parse(metadata) : {};
    const result = await apiCall("POST", "/v1/documents", { content, metadata: meta });
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_get_document",
  "Retrieve a document by its UUID from AetherDB.",
  {
    id: { type: "string", description: "Document UUID" },
  },
  async ({ id }) => {
    const result = await apiCall("GET", `/v1/documents/${id}`);
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_update_document",
  "Update an existing document's content and metadata.",
  {
    id: { type: "string", description: "Document UUID" },
    content: { type: "string", description: "New document content" },
    metadata: { type: "string", description: "Optional JSON metadata object" },
  },
  async ({ id, content, metadata }) => {
    const meta = metadata ? JSON.parse(metadata) : {};
    const result = await apiCall("PUT", `/v1/documents/${id}`, { content, metadata: meta });
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_delete_document",
  "Delete a document from AetherDB by its UUID.",
  {
    id: { type: "string", description: "Document UUID" },
  },
  async ({ id }) => {
    await apiCall("DELETE", `/v1/documents/${id}`);
    return { content: [{ type: "text", text: "Document deleted successfully." }] };
  }
);

// --- Search tools ---

server.tool(
  "aetherdb_search",
  "Perform semantic vector search across all documents. Returns results ranked by cosine similarity. Supports metadata filtering.",
  {
    query: { type: "string", description: "Search query text" },
    top_k: { type: "number", description: "Number of results (1-100, default 10)" },
    filter: { type: "string", description: "Optional JSON metadata filter (e.g. '{\"source\":\"myapp\"}')" },
    return_content: { type: "boolean", description: "Include full document content in results (default false)" },
  },
  async ({ query, top_k, filter, return_content }) => {
    const body = { query, top_k: top_k || 10, return_content: return_content || false };
    if (filter) body.filter = JSON.parse(filter);
    const result = await apiCall("POST", "/v1/search/semantic", body);
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_hybrid_search",
  "Hybrid search combining vector similarity with knowledge graph traversal. Score = (1 - graph_weight) * vector_score + graph_weight * graph_score.",
  {
    query: { type: "string", description: "Search query text" },
    top_k: { type: "number", description: "Number of results (default 10)" },
    graph_depth: { type: "number", description: "Graph traversal depth (1-5, default 2)" },
    graph_weight: { type: "number", description: "Weight of graph vs vector score (0-1, default 0.3)" },
    filter: { type: "string", description: "Optional JSON metadata filter" },
    return_content: { type: "boolean", description: "Include full document content (default false)" },
  },
  async ({ query, top_k, graph_depth, graph_weight, filter, return_content }) => {
    const body = {
      query, top_k: top_k || 10,
      graph_depth: graph_depth || 2,
      graph_weight: graph_weight ?? 0.3,
      return_content: return_content || false,
    };
    if (filter) body.filter = JSON.parse(filter);
    const result = await apiCall("POST", "/v1/search/hybrid", body);
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_fulltext_search",
  "Full-text keyword search using SQLite FTS5. Supports MATCH syntax (AND, OR, NOT, phrases). Returns snippets and BM25 ranking.",
  {
    query: { type: "string", description: "FTS5 query (e.g. 'Rust ownership', 'memory AND safety')" },
    top_k: { type: "number", description: "Number of results (default 10)" },
    return_content: { type: "boolean", description: "Include full document content (default false)" },
  },
  async ({ query, top_k, return_content }) => {
    const result = await apiCall("POST", "/v1/search/fulltext", {
      query, top_k: top_k || 10, return_content: return_content || false,
    });
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_list_documents",
  "List documents with optional filters and pagination.",
  {
    limit: { type: "number", description: "Max results (default 50, max 1000)" },
    offset: { type: "number", description: "Pagination offset (default 0)" },
    content_contains: { type: "string", description: "Filter by content substring" },
  },
  async ({ limit, offset, content_contains }) => {
    const params = new URLSearchParams();
    if (limit) params.set("limit", limit);
    if (offset) params.set("offset", offset);
    if (content_contains) params.set("content_contains", content_contains);
    const result = await apiCall("GET", `/v1/documents/list?${params}`);
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_sql_query",
  "Execute a read-only SQL query against the AetherDB database. Only SELECT/WITH allowed.",
  {
    sql: { type: "string", description: "SQL query (SELECT only)" },
  },
  async ({ sql }) => {
    const result = await apiCall("POST", "/v1/query", { sql });
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_get_versions",
  "Get version history for a document. Returns previous versions in reverse chronological order.",
  {
    id: { type: "string", description: "Document UUID" },
  },
  async ({ id }) => {
    const result = await apiCall("GET", `/v1/documents/${id}/versions`);
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

// --- Graph tools ---

server.tool(
  "aetherdb_get_entities",
  "Get extracted entities (people, organizations, concepts, etc.) from a specific document.",
  {
    document_id: { type: "string", description: "Document UUID" },
  },
  async ({ document_id }) => {
    const result = await apiCall("GET", `/v1/graph/entities?document_id=${document_id}`);
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

server.tool(
  "aetherdb_get_related",
  "Find documents related to a named entity via knowledge graph traversal.",
  {
    entity_name: { type: "string", description: "Entity name to search for" },
    depth: { type: "number", description: "Graph traversal depth (1-5, default 2)" },
  },
  async ({ entity_name, depth }) => {
    const result = await apiCall(
      "GET",
      `/v1/graph/entities/${encodeURIComponent(entity_name)}/related?depth=${depth || 2}`
    );
    return { content: [{ type: "text", text: JSON.stringify(result, null, 2) }] };
  }
);

// --- Start server ---

const transport = new StdioServerTransport();
await server.connect(transport);
