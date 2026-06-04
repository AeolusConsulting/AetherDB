/**
 * AetherDB Cloudflare Worker
 *
 * CF Workers + D1 + Workers AI (bge-base-en-v1.5) + Vectorize.
 * Auth: Bearer token via API_TOKEN secret.
 * Custom domain: configured via wrangler.toml
 * Remote MCP: /mcp endpoint (Streamable HTTP transport)
 */

import { createMcpHandler } from "agents/mcp";
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";

function createMcpServer(env, ctx) {
  const server = new McpServer({ name: "aetherdb", version: "1.0.0" });

  function fakeReq(body) {
    return { json: async () => body };
  }
  function fakeUrl(qs) {
    return { searchParams: new URLSearchParams(qs) };
  }
  async function call(fn) {
    const resp = await fn();
    if (resp instanceof Response) {
      if (resp.status === 204) return { deleted: true };
      return resp.json();
    }
    return resp;
  }

  const txt = (data) => ({ content: [{ type: "text", text: JSON.stringify(data, null, 2) }] });

  server.tool("aetherdb_create_document", "Store a document. Auto-embeds and extracts entities.", {
    content: z.string().describe("Document content"),
    metadata: z.string().optional().describe("JSON metadata object"),
    id: z.string().optional().describe("Optional client-supplied ID"),
  }, async ({ content, metadata, id }) => {
    const body = { content, metadata: metadata ? JSON.parse(metadata) : {} };
    if (id) body.id = id;
    return txt(await call(() => createDocument(fakeReq(body), env, ctx)));
  });

  server.tool("aetherdb_upsert_document", "Create or update by ID or metadata key.", {
    content: z.string().describe("Document content"),
    metadata: z.string().optional().describe("JSON metadata object"),
    id: z.string().optional().describe("Document ID"),
    metadata_key: z.string().optional().describe("Metadata key for upsert lookup"),
  }, async ({ content, metadata, id, metadata_key }) => {
    const body = { content, metadata: metadata ? JSON.parse(metadata) : {} };
    if (id) body.id = id;
    if (metadata_key) body.metadata_key = metadata_key;
    return txt(await call(() => upsertDocument(fakeReq(body), env, ctx)));
  });

  server.tool("aetherdb_get_document", "Retrieve a document by UUID.", {
    id: z.string().describe("Document UUID"),
  }, async ({ id }) => txt(await call(() => getDocument(id, env))));

  server.tool("aetherdb_delete_document", "Delete a document.", {
    id: z.string().describe("Document UUID"),
  }, async ({ id }) => txt(await call(() => deleteDocument(id, env))));

  server.tool("aetherdb_search", "Semantic vector search with optional metadata filtering.", {
    query: z.string().describe("Search query"),
    top_k: z.number().optional().describe("Results count (default 10)"),
    filter: z.string().optional().describe("JSON metadata filter"),
    return_content: z.boolean().optional().describe("Include full content"),
  }, async ({ query, top_k, filter, return_content }) => {
    const body = { query, top_k: top_k || 10, return_content: return_content || false };
    if (filter) body.filter = JSON.parse(filter);
    return txt(await call(() => semanticSearch(fakeReq(body), env)));
  });

  server.tool("aetherdb_hybrid_search", "Hybrid vector + graph search.", {
    query: z.string().describe("Search query"),
    top_k: z.number().optional().describe("Results count (default 10)"),
    graph_depth: z.number().optional().describe("Depth 0-5 (default 2)"),
    graph_weight: z.number().optional().describe("Weight 0-1 (default 0.3)"),
    filter: z.string().optional().describe("JSON metadata filter"),
    return_content: z.boolean().optional().describe("Include full content"),
  }, async ({ query, top_k, graph_depth, graph_weight, filter, return_content }) => {
    const body = { query, top_k: top_k || 10, graph_depth: graph_depth ?? 2, graph_weight: graph_weight ?? 0.3, return_content: return_content || false };
    if (filter) body.filter = JSON.parse(filter);
    return txt(await call(() => hybridSearch(fakeReq(body), env)));
  });

  server.tool("aetherdb_fulltext_search", "Full-text keyword search (FTS5).", {
    query: z.string().describe("FTS5 query"),
    top_k: z.number().optional().describe("Results count (default 10)"),
    return_content: z.boolean().optional().describe("Include full content"),
  }, async ({ query, top_k, return_content }) =>
    txt(await call(() => fulltextSearch(fakeReq({ query, top_k: top_k || 10, return_content: return_content || false }), env))));

  server.tool("aetherdb_list_documents", "List documents with filters.", {
    limit: z.number().optional().describe("Max results (default 50)"),
    offset: z.number().optional().describe("Offset"),
    content_contains: z.string().optional().describe("Content filter"),
    metadata_key: z.string().optional().describe("Metadata key filter"),
    metadata_value: z.string().optional().describe("Metadata value filter"),
  }, async ({ limit, offset, content_contains, metadata_key, metadata_value }) => {
    const p = new URLSearchParams();
    if (limit) p.set("limit", String(limit));
    if (offset) p.set("offset", String(offset));
    if (content_contains) p.set("content_contains", content_contains);
    if (metadata_key) p.set("metadata_key", metadata_key);
    if (metadata_value) p.set("metadata_value", metadata_value);
    return txt(await call(() => listDocuments(fakeUrl(p.toString()), env)));
  });

  server.tool("aetherdb_sql_query", "Read-only SQL query (SELECT/WITH).", {
    sql: z.string().describe("SQL query"),
  }, async ({ sql }) => txt(await call(() => sqlQuery(fakeReq({ sql }), env))));

  server.tool("aetherdb_get_versions", "Get version history.", {
    id: z.string().describe("Document UUID"),
  }, async ({ id }) => txt(await call(() => getVersions(id, env))));

  server.tool("aetherdb_get_entities", "Get entities from a document.", {
    document_id: z.string().describe("Document UUID"),
  }, async ({ document_id }) => txt(await call(() => getEntities(document_id, env))));

  server.tool("aetherdb_get_related", "Find related docs via graph traversal.", {
    entity_name: z.string().describe("Entity name"),
    depth: z.number().optional().describe("Depth 1-5 (default 2)"),
  }, async ({ entity_name, depth }) =>
    txt(await call(() => getRelated(entity_name, depth || 2, env))));

  return server;
}

export default {
  async fetch(request, env, ctx) {
    const url = new URL(request.url);
    const path = url.pathname;
    const method = request.method;

    // CORS preflight
    if (method === "OPTIONS") {
      return new Response(null, {
        headers: {
          "Access-Control-Allow-Origin": "*",
          "Access-Control-Allow-Methods": "GET, POST, PUT, DELETE, OPTIONS",
          "Access-Control-Allow-Headers": "Content-Type, Authorization",
          "Access-Control-Max-Age": "86400",
        },
      });
    }

    try {
      // Remote MCP server — requires same Bearer auth as REST API
      if (path === "/mcp" || path.startsWith("/mcp/")) {
        const authErr = checkAuth(request, env);
        if (authErr) return authErr;
        const mcpServer = createMcpServer(env, ctx);
        const handler = createMcpHandler(mcpServer, { route: "/mcp" });
        return handler(request, env, ctx);
      }

      // Health — no auth required
      if (path === "/health") {
        const [docs, entities, rels] = await Promise.all([
          env.DB.prepare("SELECT COUNT(*) as n FROM documents WHERE deleted = 0").first(),
          env.DB.prepare("SELECT COUNT(*) as n FROM entities").first().catch(() => ({ n: 0 })),
          env.DB.prepare("SELECT COUNT(*) as n FROM relationships").first().catch(() => ({ n: 0 })),
        ]);
        return json({
          status: "ok",
          domain: url.hostname,
          documents: docs.n,
          entities: entities.n,
          relationships: rels.n,
        });
      }

      // Admin endpoints — separate auth
      if (path.startsWith("/v1/admin/")) {
        const adminErr = checkAdmin(request, env);
        if (adminErr) return adminErr;
        if (path === "/v1/admin/backfill-embeddings" && method === "POST") {
          return await backfillEmbeddings(env);
        }
        if (path === "/v1/admin/backfill-entities" && method === "POST") {
          return await backfillEntities(env);
        }
        if (path === "/v1/admin/backfill-fts" && method === "POST") {
          return await backfillFts(env);
        }
        if (path === "/v1/admin/init-schema" && method === "POST") {
          return await initSchema(env);
        }
        if (path === "/v1/admin/cleanup" && method === "POST") {
          return await cleanup(env);
        }
        return json({ error: "not found" }, 404);
      }

      // Auth check for all other endpoints
      const authError = checkAuth(request, env);
      if (authError) return authError;

      // Rate limiting
      const rateLimitErr = await checkRateLimit(request, env);
      if (rateLimitErr) return rateLimitErr;

      // Warehouse
      if (path === "/v1/documents/bulk" && method === "POST") {
        return await bulkImport(request, env, ctx);
      }
      if (path === "/v1/documents/list" && method === "GET") {
        return await listDocuments(url, env);
      }
      if (path === "/v1/query" && method === "POST") {
        return await sqlQuery(request, env);
      }

      // Documents
      if (path === "/v1/documents/upsert" && method === "POST") {
        return await upsertDocument(request, env, ctx);
      }
      if (path === "/v1/documents" && method === "POST") {
        return await createDocument(request, env, ctx);
      }
      if (path.match(/^\/v1\/documents\/[\w-]+$/) && method === "GET") {
        const id = path.split("/").pop();
        return await getDocument(id, env);
      }
      if (path.match(/^\/v1\/documents\/[\w-]+$/) && method === "PUT") {
        const id = path.split("/").pop();
        return await updateDocument(id, request, env, ctx);
      }
      if (path.match(/^\/v1\/documents\/[\w-]+\/versions$/) && method === "GET") {
        const id = path.split("/")[3];
        return await getVersions(id, env);
      }
      if (path.match(/^\/v1\/documents\/[\w-]+$/) && method === "DELETE") {
        const id = path.split("/").pop();
        return await deleteDocument(id, env);
      }

      // Search
      if (path === "/v1/search/semantic" && method === "POST") {
        return await semanticSearch(request, env);
      }
      if (path === "/v1/search/hybrid" && method === "POST") {
        return await hybridSearch(request, env);
      }
      if (path === "/v1/search/fulltext" && method === "POST") {
        return await fulltextSearch(request, env);
      }

      // Graph
      if (path === "/v1/graph/entities" && method === "GET") {
        const docId = url.searchParams.get("document_id");
        return await getEntities(docId, env);
      }
      if (path.match(/^\/v1\/graph\/entities\/[^/]+\/related$/) && method === "GET") {
        const name = decodeURIComponent(path.split("/")[4]);
        const depth = parseInt(url.searchParams.get("depth") || "2");
        return await getRelated(name, depth, env);
      }

      // Sync
      if (path === "/v1/sync/pull" && method === "POST") {
        return await syncPull(request, env);
      }
      if (path === "/v1/sync/push" && method === "POST") {
        return await syncPush(request, env);
      }

      return json({ error: "not found" }, 404);
    } catch (e) {
      return json({ error: e.message }, 500);
    }
  },
};

// --- Auth ---

function checkAuth(request, env) {
  const tokens = [env.API_TOKEN, env.API_TOKEN_2, env.API_TOKEN_3].filter(Boolean);
  if (tokens.length === 0) return null; // No tokens configured = auth disabled

  const authHeader = request.headers.get("Authorization");
  if (!authHeader || !authHeader.startsWith("Bearer ")) {
    return json({ error: "Missing Authorization: Bearer <token> header" }, 401);
  }

  const provided = authHeader.slice(7);
  if (!tokens.includes(provided)) {
    return json({ error: "Invalid API token" }, 401);
  }

  return null; // Auth passed
}

function checkAdmin(request, env) {
  if (!env.ADMIN_TOKEN) return null;
  const authHeader = request.headers.get("Authorization");
  const provided = authHeader ? authHeader.slice(7) : "";
  if (provided !== env.ADMIN_TOKEN) {
    return json({ error: "admin access required" }, 403);
  }
  return null;
}

async function checkRateLimit(request, env) {
  const rpm = parseInt(env.RATE_LIMIT_RPM || "60");
  if (rpm <= 0) return null;

  const authHeader = request.headers.get("Authorization") || "";
  const tokenHash = authHeader.slice(0, 20) || "anon";
  const window = Math.floor(Date.now() / 60000);
  const key = `${tokenHash}:${window}`;

  try {
    const row = await env.DB.prepare(
      "INSERT INTO rate_limits (key, count, window_start) VALUES (?, 1, ?) ON CONFLICT(key) DO UPDATE SET count = count + 1 RETURNING count"
    ).bind(key, window).first();
    if (row && row.count > rpm) {
      return new Response(JSON.stringify({ error: "rate limit exceeded", retry_after_seconds: 60 - (Math.floor(Date.now() / 1000) % 60) }), {
        status: 429,
        headers: { "Content-Type": "application/json", "Retry-After": String(60 - (Math.floor(Date.now() / 1000) % 60)), "Access-Control-Allow-Origin": "*" },
      });
    }
  } catch (e) {
    // Table might not exist yet — pass through
  }
  return null;
}

// --- Helpers ---

function json(data, status = 200) {
  return new Response(JSON.stringify(data), {
    status,
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
    },
  });
}

function generateId() {
  return crypto.randomUUID();
}

async function hashContent(content) {
  const encoder = new TextEncoder();
  const data = encoder.encode(content);
  const hash = await crypto.subtle.digest("SHA-256", data);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

// --- Document CRUD ---

async function embedText(text, env) {
  const resp = await env.AI.run("@cf/baai/bge-base-en-v1.5", { text: [text] });
  return resp.data[0];
}

async function embedBatch(texts, env) {
  const resp = await env.AI.run("@cf/baai/bge-base-en-v1.5", { text: texts });
  return resp.data;
}

// --- Entity Extraction ---

const ENTITY_TYPES = new Set(["person", "organization", "concept", "location", "event", "technology", "other"]);
const RELATIONSHIP_TYPES = new Set(["related_to", "works_at", "located_in", "part_of", "created_by", "uses"]);
const EXTRACTION_MODEL = "@cf/zai-org/glm-4.7-flash";

const EXTRACTION_PROMPT = `Extract entities and relationships from the following text. Return ONLY valid JSON with this exact structure:
{"entities": [{"name": "EntityName", "entity_type": "person|organization|concept|location|event|technology|other"}], "relationships": [{"source": "EntityName1", "target": "EntityName2", "relationship_type": "related_to|works_at|located_in|part_of|created_by|uses"}]}
If no entities are found, return {"entities": [], "relationships": []}.

Text:
`;

function parseExtraction(raw) {
  try {
    const start = raw.indexOf("{");
    const end = raw.lastIndexOf("}");
    if (start === -1 || end === -1) return { entities: [], relationships: [] };

    const parsed = JSON.parse(raw.substring(start, end + 1));
    if (!Array.isArray(parsed.entities)) return { entities: [], relationships: [] };

    const entities = parsed.entities.filter(
      (e) => e.name && typeof e.name === "string" && ENTITY_TYPES.has(e.entity_type)
    );
    const entityNames = new Set(entities.map((e) => e.name));

    const relationships = Array.isArray(parsed.relationships)
      ? parsed.relationships.filter(
          (r) =>
            r.source && r.target &&
            entityNames.has(r.source) && entityNames.has(r.target) &&
            RELATIONSHIP_TYPES.has(r.relationship_type)
        )
      : [];

    return { entities, relationships };
  } catch {
    return { entities: [], relationships: [] };
  }
}

async function extractEntities(content, env) {
  const truncated = content.substring(0, 4000);
  const response = await env.AI.run(EXTRACTION_MODEL, {
    messages: [
      { role: "system", content: "You are an entity extraction assistant. Always respond with valid JSON only." },
      { role: "user", content: EXTRACTION_PROMPT + truncated },
    ],
    temperature: 0.0,
  });
  const raw = response.response || response.choices?.[0]?.message?.content || "";
  return parseExtraction(raw);
}

async function storeEntities(documentId, extraction, env) {
  const now = Date.now();
  const entityMap = {};

  for (const entity of extraction.entities) {
    const id = generateId();
    entityMap[entity.name] = id;
    await env.DB.prepare(
      "INSERT INTO entities (id, name, entity_type, document_id, created_at) VALUES (?, ?, ?, ?, ?)"
    ).bind(id, entity.name, entity.entity_type, documentId, now).run();
  }

  let relCount = 0;
  for (const rel of extraction.relationships) {
    const sourceId = entityMap[rel.source];
    const targetId = entityMap[rel.target];
    if (sourceId && targetId) {
      await env.DB.prepare(
        "INSERT INTO relationships (id, source_entity_id, target_entity_id, relationship_type, weight, document_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
      ).bind(generateId(), sourceId, targetId, rel.relationship_type, 1.0, documentId, now).run();
      relCount++;
    }
  }

  return { entity_count: extraction.entities.length, relationship_count: relCount };
}

async function deleteEntitiesForDocument(documentId, env) {
  await env.DB.prepare("DELETE FROM relationships WHERE document_id = ?").bind(documentId).run();
  await env.DB.prepare("DELETE FROM entities WHERE document_id = ?").bind(documentId).run();
}

function buildVectorMetadata(content, metadata) {
  const meta = { content_preview: content.substring(0, 200) };
  if (metadata) {
    const parsed = typeof metadata === "string" ? JSON.parse(metadata) : metadata;
    for (const [k, v] of Object.entries(parsed)) {
      if (typeof v === "string" || typeof v === "number" || typeof v === "boolean") {
        meta[k] = v;
      }
    }
  }
  return meta;
}

// --- Document CRUD ---

async function createDocument(request, env, ctx) {
  const { id: clientId, content, metadata, deduplicate } = await request.json();
  if (!content || content.length === 0) {
    return json({ error: "content must not be empty" }, 400);
  }
  if (content.length > 1_048_576) {
    return json({ error: "content too large" }, 400);
  }

  const contentHash = await hashContent(content);

  if (deduplicate !== false) {
    const existing = await env.DB.prepare(
      "SELECT id, created_at FROM documents WHERE content_hash = ? AND deleted = 0"
    ).bind(contentHash).first();
    if (existing) {
      return json({
        id: existing.id,
        created_at: new Date(existing.created_at).toISOString(),
        duplicate: true,
      }, 200);
    }
  }

  const id = clientId || generateId();
  const now = Date.now();
  const meta = JSON.stringify(metadata || {});

  await env.DB.prepare(
    `INSERT INTO documents (id, content, metadata, content_hash, created_at, updated_at)
     VALUES (?, ?, ?, ?, ?, ?)`
  )
    .bind(id, content, meta, contentHash, now, now)
    .run();

  await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(id, content).run().catch(() => {});

  const vector = await embedText(content.substring(0, 2048), env);
  await env.VECTORIZE.upsert([{ id, values: vector, metadata: buildVectorMetadata(content, metadata) }]);

  if (ctx) {
    ctx.waitUntil((async () => {
      try {
        const extraction = await extractEntities(content, env);
        if (extraction.entities.length > 0) await storeEntities(id, extraction, env);
      } catch (e) { /* best-effort */ }
    })());
  }

  return json({ id, created_at: new Date(now).toISOString(), embedded: true, entities: "processing" }, 201);
}

async function upsertDocument(request, env, ctx) {
  const { id, content, metadata, metadata_key } = await request.json();
  if (!content || content.length === 0) {
    return json({ error: "content must not be empty" }, 400);
  }
  if (content.length > 1_048_576) {
    return json({ error: "content too large" }, 400);
  }

  let existingId = null;

  if (id) {
    const row = await env.DB.prepare("SELECT id FROM documents WHERE id = ? AND deleted = 0").bind(id).first();
    if (row) existingId = row.id;
  } else if (metadata_key && metadata) {
    const keyValue = typeof metadata === "object" ? metadata[metadata_key] : null;
    if (keyValue) {
      const row = await env.DB.prepare(
        "SELECT id FROM documents WHERE json_extract(metadata, ?) = ? AND deleted = 0"
      ).bind(`$.${metadata_key}`, String(keyValue)).first();
      if (row) existingId = row.id;
    }
  }

  if (existingId) {
    const now = Date.now();
    const contentHash = await hashContent(content);
    const meta = JSON.stringify(metadata || {});

    const old = await env.DB.prepare(
      "SELECT content, metadata, content_hash, updated_at FROM documents WHERE id = ?"
    ).bind(existingId).first();

    await env.DB.prepare(
      "UPDATE documents SET content = ?, metadata = ?, content_hash = ?, updated_at = ? WHERE id = ?"
    ).bind(content, meta, contentHash, now, existingId).run();

    if (old) {
      await env.DB.prepare(
        "INSERT INTO document_versions (id, document_id, content, metadata, content_hash, created_at) VALUES (?, ?, ?, ?, ?, ?)"
      ).bind(generateId(), existingId, old.content, old.metadata, old.content_hash, old.updated_at).run().catch(() => {});
    }

    await env.DB.prepare("DELETE FROM documents_fts WHERE id = ?").bind(existingId).run().catch(() => {});
    await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(existingId, content).run().catch(() => {});

    const vector = await embedText(content.substring(0, 2048), env);
    await env.VECTORIZE.upsert([{ id: existingId, values: vector, metadata: buildVectorMetadata(content, metadata) }]);

    await deleteEntitiesForDocument(existingId, env);
    if (ctx) {
      ctx.waitUntil((async () => {
        try {
          const extraction = await extractEntities(content, env);
          if (extraction.entities.length > 0) await storeEntities(existingId, extraction, env);
        } catch (e) { /* best-effort */ }
      })());
    }

    return json({ id: existingId, updated_at: new Date(now).toISOString(), embedded: true, entities: "processing", upsert: "updated" });
  }

  const newId = id || generateId();
  const now = Date.now();
  const contentHash = await hashContent(content);
  const meta = JSON.stringify(metadata || {});

  await env.DB.prepare(
    "INSERT INTO documents (id, content, metadata, content_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
  ).bind(newId, content, meta, contentHash, now, now).run();

  await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(newId, content).run().catch(() => {});

  const vector = await embedText(content.substring(0, 2048), env);
  await env.VECTORIZE.upsert([{ id: newId, values: vector, metadata: buildVectorMetadata(content, metadata) }]);

  if (ctx) {
    ctx.waitUntil((async () => {
      try {
        const extraction = await extractEntities(content, env);
        if (extraction.entities.length > 0) await storeEntities(newId, extraction, env);
      } catch (e) { /* best-effort */ }
    })());
  }

  return json({ id: newId, created_at: new Date(now).toISOString(), embedded: true, entities: "processing", upsert: "created" }, 201);
}

async function getDocument(id, env) {
  const row = await env.DB.prepare(
    "SELECT id, content, metadata, created_at, updated_at FROM documents WHERE id = ? AND deleted = 0"
  )
    .bind(id)
    .first();

  if (!row) return json({ error: "not found" }, 404);

  return json({
    id: row.id,
    content: row.content,
    metadata: JSON.parse(row.metadata),
    created_at: new Date(row.created_at).toISOString(),
    updated_at: new Date(row.updated_at).toISOString(),
  });
}

async function getVersions(id, env) {
  const rows = await env.DB.prepare(
    "SELECT id, content, metadata, content_hash, created_at FROM document_versions WHERE document_id = ? ORDER BY created_at DESC"
  ).bind(id).all().catch(() => ({ results: [] }));

  const versions = rows.results.map((r) => ({
    version_id: r.id,
    content: r.content,
    metadata: JSON.parse(r.metadata),
    content_hash: r.content_hash,
    created_at: new Date(r.created_at).toISOString(),
  }));

  return json({ document_id: id, versions, count: versions.length });
}

async function updateDocument(id, request, env, ctx) {
  const { content, metadata } = await request.json();
  if (!content || content.length === 0) {
    return json({ error: "content must not be empty" }, 400);
  }
  if (content.length > 1_048_576) {
    return json({ error: "content too large" }, 400);
  }

  const old = await env.DB.prepare(
    "SELECT content, metadata, content_hash, updated_at FROM documents WHERE id = ? AND deleted = 0"
  ).bind(id).first();
  if (!old) return json({ error: "not found" }, 404);

  const now = Date.now();
  const contentHash = await hashContent(content);
  const meta = JSON.stringify(metadata || {});

  await env.DB.prepare(
    "UPDATE documents SET content = ?, metadata = ?, content_hash = ?, updated_at = ? WHERE id = ? AND deleted = 0"
  )
    .bind(content, meta, contentHash, now, id)
    .run();

  await env.DB.prepare(
    "INSERT INTO document_versions (id, document_id, content, metadata, content_hash, created_at) VALUES (?, ?, ?, ?, ?, ?)"
  ).bind(generateId(), id, old.content, old.metadata, old.content_hash, old.updated_at).run().catch(() => {});

  await env.DB.prepare("DELETE FROM documents_fts WHERE id = ?").bind(id).run().catch(() => {});
  await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(id, content).run().catch(() => {});

  const vector = await embedText(content.substring(0, 2048), env);
  await env.VECTORIZE.upsert([{ id, values: vector, metadata: buildVectorMetadata(content, metadata) }]);

  await deleteEntitiesForDocument(id, env);
  if (ctx) {
    ctx.waitUntil((async () => {
      try {
        const extraction = await extractEntities(content, env);
        if (extraction.entities.length > 0) await storeEntities(id, extraction, env);
      } catch (e) { /* best-effort */ }
    })());
  }

  return json({ id, updated_at: new Date(now).toISOString(), embedded: true, entities: "processing" });
}

async function deleteDocument(id, env) {
  const result = await env.DB.prepare(
    "UPDATE documents SET deleted = 1, updated_at = ? WHERE id = ? AND deleted = 0"
  )
    .bind(Date.now(), id)
    .run();

  if (result.meta.changes === 0) return json({ error: "not found" }, 404);

  await env.DB.prepare("DELETE FROM documents_fts WHERE id = ?").bind(id).run().catch(() => {});
  await deleteEntitiesForDocument(id, env);
  await env.VECTORIZE.deleteByIds([id]);

  return new Response(null, {
    status: 204,
    headers: { "Access-Control-Allow-Origin": "*" },
  });
}

// --- Search (Workers AI + Vectorize) ---

async function semanticSearch(request, env) {
  const { query, top_k, return_content, filter } = await request.json();
  if (!query || query.length === 0) {
    return json({ error: "query must not be empty" }, 400);
  }

  const k = Math.min(Math.max(top_k || 10, 1), 100);

  const queryVector = await embedText(query, env);
  const queryOpts = { topK: k, returnMetadata: "all" };
  if (filter && typeof filter === "object") queryOpts.filter = filter;
  const matches = await env.VECTORIZE.query(queryVector, queryOpts);

  let results = (matches.matches || []).map((m) => {
    const { content_preview, ...vectorMeta } = m.metadata || {};
    return {
      document_id: m.id,
      score: m.score,
      content_preview: content_preview || "",
      metadata: Object.keys(vectorMeta).length > 0 ? vectorMeta : null,
    };
  });

  if (return_content && results.length > 0) {
    const ids = results.map((r) => r.document_id);
    const placeholders = ids.map(() => "?").join(",");
    const rows = await env.DB.prepare(
      `SELECT id, content, metadata FROM documents WHERE id IN (${placeholders}) AND deleted = 0`
    ).bind(...ids).all();
    const docMap = Object.fromEntries(rows.results.map((r) => [r.id, r]));
    results = results.map((r) => ({
      ...r,
      content: docMap[r.document_id]?.content || null,
      metadata: docMap[r.document_id] ? JSON.parse(docMap[r.document_id].metadata) : null,
    }));
  }

  return json({ results, count: results.length });
}

async function hybridSearch(request, env) {
  const { query, top_k, graph_depth, graph_weight, return_content, filter } = await request.json();
  if (!query || query.length === 0) {
    return json({ error: "query must not be empty" }, 400);
  }

  const k = Math.min(Math.max(top_k || 10, 1), 100);
  const depth = Math.min(Math.max(graph_depth ?? 2, 0), 5);
  const gw = Math.min(Math.max(graph_weight ?? 0.3, 0.0), 1.0);

  const queryVector = await embedText(query, env);
  const queryOpts = { topK: k * 2, returnMetadata: "all" };
  if (filter && typeof filter === "object") queryOpts.filter = filter;
  const matches = await env.VECTORIZE.query(queryVector, queryOpts);

  const scores = {};
  for (const m of matches.matches || []) {
    scores[m.id] = { vector_score: m.score, graph_score: 0 };
  }

  const words = query.split(/\s+/).filter((w) => w.length >= 3);
  for (const word of words) {
    try {
      const rows = await env.DB.prepare(`
        WITH RECURSIVE reachable(entity_id, hop) AS (
          SELECT id, 0 FROM entities WHERE name = ? COLLATE NOCASE
          UNION ALL
          SELECT CASE WHEN r.source_entity_id = re.entity_id THEN r.target_entity_id ELSE r.source_entity_id END, re.hop + 1
          FROM reachable re
          JOIN relationships r ON r.source_entity_id = re.entity_id OR r.target_entity_id = re.entity_id
          WHERE re.hop < ?
        )
        SELECT DISTINCT e.document_id, MIN(re.hop) as min_hop
        FROM reachable re JOIN entities e ON e.id = re.entity_id
        WHERE e.document_id IS NOT NULL
        GROUP BY e.document_id
      `).bind(word, depth).all();

      for (const row of rows.results) {
        const gs = 1.0 / (1.0 + row.min_hop);
        if (!scores[row.document_id]) {
          scores[row.document_id] = { vector_score: 0, graph_score: gs };
        } else if (gs > scores[row.document_id].graph_score) {
          scores[row.document_id].graph_score = gs;
        }
      }
    } catch (e) { /* skip failed graph queries */ }
  }

  let results = Object.entries(scores).map(([docId, s]) => ({
    document_id: docId,
    score: (1 - gw) * s.vector_score + gw * s.graph_score,
    vector_score: s.vector_score,
    graph_score: s.graph_score,
  }));

  results.sort((a, b) => b.score - a.score);
  results = results.slice(0, k);

  if (return_content && results.length > 0) {
    const ids = results.map((r) => r.document_id);
    const placeholders = ids.map(() => "?").join(",");
    const rows = await env.DB.prepare(
      `SELECT id, content, metadata FROM documents WHERE id IN (${placeholders}) AND deleted = 0`
    ).bind(...ids).all();
    const docMap = Object.fromEntries(rows.results.map((r) => [r.id, r]));
    results = results.map((r) => ({
      ...r,
      content: docMap[r.document_id]?.content || null,
      metadata: docMap[r.document_id] ? JSON.parse(docMap[r.document_id].metadata) : null,
      content_preview: (docMap[r.document_id]?.content || "").substring(0, 200),
    }));
  } else {
    const vecMetaMap = Object.fromEntries(
      (matches.matches || []).map((m) => {
        const { content_preview, ...rest } = m.metadata || {};
        return [m.id, { content_preview: content_preview || "", metadata: Object.keys(rest).length > 0 ? rest : null }];
      })
    );
    results = results.map((r) => ({
      ...r,
      content_preview: vecMetaMap[r.document_id]?.content_preview || "",
      metadata: vecMetaMap[r.document_id]?.metadata || null,
    }));
  }

  return json({ results, count: results.length });
}

async function fulltextSearch(request, env) {
  const { query, top_k, return_content } = await request.json();
  if (!query || query.length === 0) {
    return json({ error: "query must not be empty" }, 400);
  }

  const k = Math.min(Math.max(top_k || 10, 1), 100);

  try {
    const rows = await env.DB.prepare(`
      SELECT f.id, snippet(documents_fts, 1, '<b>', '</b>', '...', 32) as snippet, rank
      FROM documents_fts f
      JOIN documents d ON d.id = f.id AND d.deleted = 0
      WHERE documents_fts MATCH ?
      ORDER BY rank
      LIMIT ?
    `).bind(query, k).all();

    let results = rows.results.map((r) => ({
      document_id: r.id,
      snippet: r.snippet,
      rank: r.rank,
    }));

    if (return_content && results.length > 0) {
      const ids = results.map((r) => r.document_id);
      const placeholders = ids.map(() => "?").join(",");
      const docs = await env.DB.prepare(
        `SELECT id, content, metadata FROM documents WHERE id IN (${placeholders}) AND deleted = 0`
      ).bind(...ids).all();
      const docMap = Object.fromEntries(docs.results.map((r) => [r.id, r]));
      results = results.map((r) => ({
        ...r,
        content: docMap[r.document_id]?.content || null,
        metadata: docMap[r.document_id] ? JSON.parse(docMap[r.document_id].metadata) : null,
      }));
    }

    return json({ results, count: results.length });
  } catch (e) {
    if (e.message?.includes("no such table")) {
      return json({ error: "FTS index not initialized. Run POST /v1/admin/backfill-fts first." }, 400);
    }
    return json({ error: e.message }, 400);
  }
}

// --- Graph ---

async function getEntities(docId, env) {
  if (!docId) return json({ error: "document_id required" }, 400);

  const rows = await env.DB.prepare(
    "SELECT id, name, entity_type, document_id, created_at FROM entities WHERE document_id = ?"
  )
    .bind(docId)
    .all();

  const entities = rows.results.map((r) => ({
    id: r.id,
    name: r.name,
    entity_type: r.entity_type,
    document_id: r.document_id,
    created_at: new Date(r.created_at).toISOString(),
  }));

  return json({ entities });
}

async function getRelated(name, depth, env) {
  if (depth > 5) return json({ error: "depth must be <= 5" }, 400);

  const rows = await env.DB.prepare(`
    WITH RECURSIVE reachable(entity_id, hop) AS (
      SELECT id, 0 FROM entities WHERE name = ? COLLATE NOCASE
      UNION ALL
      SELECT CASE WHEN r.source_entity_id = re.entity_id THEN r.target_entity_id ELSE r.source_entity_id END, re.hop + 1
      FROM reachable re
      JOIN relationships r ON r.source_entity_id = re.entity_id OR r.target_entity_id = re.entity_id
      WHERE re.hop < ?
    )
    SELECT DISTINCT e.document_id, MIN(re.hop) as min_hop
    FROM reachable re JOIN entities e ON e.id = re.entity_id
    WHERE e.document_id IS NOT NULL
    GROUP BY e.document_id
    ORDER BY min_hop ASC
  `).bind(name, depth).all();

  const documents = rows.results.map((r) => ({
    document_id: r.document_id,
    entity_name: name,
    hop_count: r.min_hop,
  }));

  return json({ documents });
}

// --- Sync ---

async function syncPull(request, env) {
  const { since } = await request.json();
  const wallMs = since?.wall_ms || 0;
  const counter = since?.counter || 0;

  const rows = await env.DB.prepare(
    `SELECT id, content, metadata, content_hash, hlc_wall_ms, hlc_counter, origin_node, deleted
     FROM documents
     WHERE (hlc_wall_ms > ?) OR (hlc_wall_ms = ? AND hlc_counter > ?)
     ORDER BY hlc_wall_ms ASC, hlc_counter ASC`
  )
    .bind(wallMs, wallMs, counter)
    .all();

  const changes = rows.results.map((r) => ({
    document_id: r.id,
    content: r.content,
    metadata: JSON.parse(r.metadata),
    content_hash: r.content_hash,
    hlc_ts: { wall_ms: r.hlc_wall_ms, counter: r.hlc_counter },
    origin_node: r.origin_node,
    deleted: r.deleted === 1,
  }));

  return json({
    changes,
    server_hlc: { wall_ms: Date.now(), counter: 0 },
  });
}

async function syncPush(request, env) {
  const { changes } = await request.json();
  let accepted = 0;
  let rejected = 0;
  const toEmbed = [];
  const toDelete = [];

  for (const change of changes || []) {
    const now = Date.now();
    const meta = JSON.stringify(change.metadata || {});

    await env.DB.prepare(
      `INSERT OR REPLACE INTO documents
         (id, content, metadata, content_hash, created_at, updated_at, hlc_wall_ms, hlc_counter, origin_node, deleted)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`
    )
      .bind(
        change.document_id,
        change.content,
        meta,
        change.content_hash,
        now,
        now,
        change.hlc_ts?.wall_ms || 0,
        change.hlc_ts?.counter || 0,
        change.origin_node || "",
        change.deleted ? 1 : 0
      )
      .run();
    accepted++;

    if (change.deleted) {
      toDelete.push(change.document_id);
    } else {
      toEmbed.push({ id: change.document_id, content: change.content, metadata: change.metadata });
      await env.DB.prepare("DELETE FROM documents_fts WHERE id = ?").bind(change.document_id).run().catch(() => {});
      await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(change.document_id, change.content).run().catch(() => {});
    }
  }

  if (toDelete.length > 0) {
    for (const id of toDelete) {
      await env.DB.prepare("DELETE FROM documents_fts WHERE id = ?").bind(id).run().catch(() => {});
    }
    await env.VECTORIZE.deleteByIds(toDelete);
  }

  for (let i = 0; i < toEmbed.length; i += 100) {
    const batch = toEmbed.slice(i, i + 100);
    const texts = batch.map((d) => d.content.substring(0, 2048));
    const vectors = await embedBatch(texts, env);
    const vectorRecords = batch.map((d, j) => ({
      id: d.id,
      values: vectors[j],
      metadata: buildVectorMetadata(d.content, d.metadata),
    }));
    await env.VECTORIZE.upsert(vectorRecords);
  }

  return json({
    accepted,
    rejected,
    embedded: toEmbed.length,
    server_hlc: { wall_ms: Date.now(), counter: 0 },
  });
}

// --- Warehouse: Bulk Import ---

async function bulkImport(request, env, ctx) {
  const body = await request.json();
  let items, extractFlag;
  if (Array.isArray(body)) {
    items = body;
    extractFlag = false;
  } else {
    items = body.documents;
    extractFlag = body.extract_entities === true;
    if (!Array.isArray(items)) return json({ error: "expected array or {documents: [], extract_entities: bool}" }, 400);
  }

  if (items.length === 0) return json({ error: "expected non-empty array" }, 400);
  if (items.length > 1000) return json({ error: "max 1000 items per batch" }, 400);

  const created = [];
  const errors = [];
  const validItems = [];

  for (let i = 0; i < items.length; i++) {
    const item = items[i];
    if (!item.content || item.content.length === 0) {
      errors.push({ index: i, error: "empty content" });
      continue;
    }
    if (item.content.length > 1_048_576) {
      errors.push({ index: i, error: "content too large" });
      continue;
    }

    const id = crypto.randomUUID();
    const now = Date.now();
    const contentHash = await hashContent(item.content);
    const meta = JSON.stringify(item.metadata || {});

    try {
      await env.DB.prepare(
        "INSERT INTO documents (id, content, metadata, content_hash, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
      )
        .bind(id, item.content, meta, contentHash, now, now)
        .run();
      await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(id, item.content).run().catch(() => {});
      created.push({ id, created_at: new Date(now).toISOString() });
      validItems.push({ id, content: item.content, metadata: item.metadata });
    } catch (e) {
      errors.push({ index: i, error: e.message });
    }
  }

  for (let i = 0; i < validItems.length; i += 100) {
    const batch = validItems.slice(i, i + 100);
    const texts = batch.map((item) => item.content.substring(0, 2048));
    const vectors = await embedBatch(texts, env);
    const vectorRecords = batch.map((item, j) => ({
      id: item.id,
      values: vectors[j],
      metadata: buildVectorMetadata(item.content, item.metadata),
    }));
    await env.VECTORIZE.upsert(vectorRecords);
  }

  if (extractFlag && validItems.length > 0 && ctx) {
    ctx.waitUntil((async () => {
      for (const item of validItems) {
        try {
          const extraction = await extractEntities(item.content, env);
          if (extraction.entities.length > 0) await storeEntities(item.id, extraction, env);
        } catch (e) { /* skip */ }
      }
    })());
  }

  return json({
    created: created.length,
    errors: errors.length,
    embedded: created.length,
    entities_extraction: extractFlag ? "background" : "skipped",
    documents: created,
    error_details: errors,
  });
}

// --- Warehouse: List / Filter ---

async function listDocuments(url, env) {
  const limit = Math.min(parseInt(url.searchParams.get("limit") || "50"), 1000);
  const offset = parseInt(url.searchParams.get("offset") || "0");
  const contentContains = url.searchParams.get("content_contains");
  const createdAfter = url.searchParams.get("created_after");
  const createdBefore = url.searchParams.get("created_before");
  const metadataKey = url.searchParams.get("metadata_key");
  const metadataValue = url.searchParams.get("metadata_value");

  let where = "deleted = 0";
  const params = [];

  if (contentContains) {
    params.push(`%${contentContains}%`);
    where += ` AND content LIKE ?`;
  }
  if (createdAfter) {
    params.push(parseInt(createdAfter));
    where += ` AND created_at >= ?`;
  }
  if (createdBefore) {
    params.push(parseInt(createdBefore));
    where += ` AND created_at <= ?`;
  }
  if (metadataKey && metadataValue) {
    params.push(`$.${metadataKey}`);
    params.push(metadataValue);
    where += ` AND json_extract(metadata, ?) = ?`;
  }

  const sql = `SELECT id, content, metadata, created_at, updated_at FROM documents WHERE ${where} ORDER BY created_at DESC LIMIT ? OFFSET ?`;
  const countSql = `SELECT COUNT(*) as total FROM documents WHERE ${where}`;

  const allParams = [...params, limit, offset];

  const rows = await env.DB.prepare(sql).bind(...allParams).all();
  const countRows = await env.DB.prepare(countSql).bind(...params).first();

  const documents = rows.results.map((r) => ({
    id: r.id,
    content: r.content,
    metadata: JSON.parse(r.metadata),
    created_at: new Date(r.created_at).toISOString(),
    updated_at: new Date(r.updated_at).toISOString(),
  }));

  return json({
    documents,
    total: countRows?.total || documents.length,
    limit,
    offset,
  });
}

// --- Admin: Backfill Embeddings ---

async function backfillEmbeddings(env) {
  const rows = await env.DB.prepare(
    "SELECT id, content, metadata FROM documents WHERE deleted = 0 ORDER BY created_at ASC"
  ).all();

  const docs = rows.results;
  let embedded = 0;

  for (let i = 0; i < docs.length; i += 100) {
    const batch = docs.slice(i, i + 100);
    const texts = batch.map((d) => d.content.substring(0, 2048));
    const vectors = await embedBatch(texts, env);
    const vectorRecords = batch.map((d, j) => ({
      id: d.id,
      values: vectors[j],
      metadata: buildVectorMetadata(d.content, d.metadata),
    }));
    await env.VECTORIZE.upsert(vectorRecords);
    embedded += batch.length;
  }

  return json({ total: docs.length, embedded });
}

async function backfillEntities(env) {
  const rows = await env.DB.prepare(`
    SELECT d.id, d.content FROM documents d
    LEFT JOIN entities e ON d.id = e.document_id
    WHERE d.deleted = 0 AND e.document_id IS NULL
    ORDER BY d.created_at ASC LIMIT 50
  `).all();

  let extracted = 0;
  let failed = 0;

  for (const doc of rows.results) {
    try {
      const extraction = await extractEntities(doc.content, env);
      if (extraction.entities.length > 0) {
        await storeEntities(doc.id, extraction, env);
        extracted++;
      }
    } catch (e) {
      failed++;
    }
  }

  return json({ total: rows.results.length, extracted, failed });
}

async function initSchema(env) {
  await env.DB.prepare("CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(id UNINDEXED, content)").run();
  await env.DB.prepare(`CREATE TABLE IF NOT EXISTS document_versions (
    id TEXT PRIMARY KEY, document_id TEXT NOT NULL, content TEXT NOT NULL,
    metadata TEXT DEFAULT '{}', content_hash TEXT, created_at INTEGER
  )`).run();
  await env.DB.prepare("CREATE INDEX IF NOT EXISTS idx_versions_document ON document_versions(document_id)").run();
  await env.DB.prepare("CREATE INDEX IF NOT EXISTS idx_documents_content_hash ON documents(content_hash)").run();
  await env.DB.prepare(`CREATE TABLE IF NOT EXISTS rate_limits (
    key TEXT PRIMARY KEY, count INTEGER DEFAULT 0, window_start INTEGER
  )`).run();
  return json({ status: "ok", tables: ["documents_fts", "document_versions", "rate_limits"], indexes: ["idx_documents_content_hash"] });
}

async function cleanup(env) {
  const currentWindow = Math.floor(Date.now() / 60000);
  const rateResult = await env.DB.prepare(
    "DELETE FROM rate_limits WHERE window_start < ?"
  ).bind(currentWindow - 1).run();

  const deletedDocs = await env.DB.prepare(
    "SELECT id FROM documents WHERE deleted = 1"
  ).all();
  let purgedDocs = 0;
  for (const doc of deletedDocs.results) {
    await env.DB.prepare("DELETE FROM relationships WHERE document_id = ?").bind(doc.id).run().catch(() => {});
    await env.DB.prepare("DELETE FROM entities WHERE document_id = ?").bind(doc.id).run().catch(() => {});
    await env.DB.prepare("DELETE FROM document_versions WHERE document_id = ?").bind(doc.id).run().catch(() => {});
    await env.DB.prepare("DELETE FROM documents_fts WHERE id = ?").bind(doc.id).run().catch(() => {});
    await env.DB.prepare("DELETE FROM documents WHERE id = ?").bind(doc.id).run();
    purgedDocs++;
  }

  const oldEmbeddings = await env.DB.prepare(
    "SELECT COUNT(*) as n FROM sqlite_master WHERE type='table' AND name='embeddings'"
  ).first();
  let droppedEmbeddings = false;
  if (oldEmbeddings?.n > 0) {
    await env.DB.prepare("DROP TABLE embeddings").run();
    droppedEmbeddings = true;
  }

  return json({
    rate_limits_purged: rateResult.meta.changes,
    soft_deleted_docs_purged: purgedDocs,
    embeddings_table_dropped: droppedEmbeddings,
  });
}

async function backfillFts(env) {
  await env.DB.prepare("CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(id UNINDEXED, content)").run();
  await env.DB.prepare("DELETE FROM documents_fts").run();

  const rows = await env.DB.prepare(
    "SELECT id, content FROM documents WHERE deleted = 0 ORDER BY created_at ASC"
  ).all();

  let indexed = 0;
  for (const doc of rows.results) {
    await env.DB.prepare("INSERT INTO documents_fts (id, content) VALUES (?, ?)").bind(doc.id, doc.content).run();
    indexed++;
  }

  return json({ total: rows.results.length, indexed });
}

// --- Warehouse: SQL Query ---

async function sqlQuery(request, env) {
  const { sql } = await request.json();
  if (!sql) return json({ error: "sql field required" }, 400);

  const trimmed = sql.trim().replace(/;+\s*$/, "");
  if (trimmed.includes(";")) {
    return json({ error: "only single statements allowed" }, 400);
  }

  const lower = trimmed.toLowerCase();
  if (!lower.startsWith("select") && !lower.startsWith("with")) {
    return json({ error: "only SELECT/WITH queries allowed" }, 400);
  }

  const forbidden = /\b(insert|update|delete|drop|alter|create|replace|attach|detach|pragma|reindex|vacuum|savepoint|release|begin|commit|rollback|grant|revoke)\b/i;
  if (forbidden.test(trimmed)) {
    return json({ error: "write/admin operations not allowed" }, 400);
  }

  try {
    const rows = await env.DB.prepare(trimmed).all();
    return json({ rows: rows.results, count: rows.results.length });
  } catch (e) {
    return json({ error: e.message }, 400);
  }
}
