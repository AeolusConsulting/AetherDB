/**
 * AetherDB TypeScript Client SDK
 *
 * Usage:
 *   import { AetherDB } from './aetherdb';
 *   const db = new AetherDB('https://your-aetherdb.workers.dev', 'your-api-key');
 *   const doc = await db.createDocument('Hello world', { source: 'test' });
 *   const results = await db.search('Hello');
 */

export interface CreateDocumentResponse {
  id: string;
  created_at: string;
  embedded?: boolean;
  entities?: string;
  duplicate?: boolean;
}

export interface DocumentDto {
  id: string;
  content: string;
  metadata: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface SearchHit {
  document_id: string;
  score: number;
  content_preview: string;
  content?: string;
  metadata?: Record<string, unknown>;
}

export interface SearchResponse {
  results: SearchHit[];
  count: number;
}

export interface HybridSearchHit {
  document_id: string;
  score: number;
  vector_score: number;
  graph_score: number;
  content_preview: string;
  content?: string;
  metadata?: Record<string, unknown>;
}

export interface HybridSearchResponse {
  results: HybridSearchHit[];
  count: number;
}

export interface FulltextHit {
  document_id: string;
  snippet: string;
  rank: number;
  content?: string;
  metadata?: Record<string, unknown>;
}

export interface FulltextResponse {
  results: FulltextHit[];
  count: number;
}

export interface EntityDto {
  id: string;
  name: string;
  entity_type: string;
  document_id: string;
  created_at: string;
}

export interface VersionDto {
  version_id: string;
  content: string;
  metadata: Record<string, unknown>;
  content_hash: string;
  created_at: string;
}

export interface ListDocumentsResponse {
  documents: DocumentDto[];
  total: number;
  limit: number;
  offset: number;
}

export interface BulkImportResponse {
  created: number;
  errors: number;
  embedded: number;
  entities_extraction: string;
  documents: Array<{ id: string; created_at: string }>;
  error_details: Array<{ index: number; error: string }>;
}

export interface HlcTimestamp {
  wall_ms: number;
  counter: number;
}

export interface Change {
  document_id: string;
  content: string;
  metadata: Record<string, unknown>;
  content_hash: string;
  hlc_ts: HlcTimestamp;
  origin_node: string;
  deleted: boolean;
}

export interface SyncPullResponse {
  changes: Change[];
  server_hlc: HlcTimestamp;
}

export interface SyncPushResponse {
  accepted: number;
  rejected: number;
  server_hlc: HlcTimestamp;
}

export interface HealthResponse {
  status: string;
  domain: string;
  documents: number;
  entities: number;
  relationships: number;
}

export class AetherDB {
  private baseUrl: string;
  private apiKey?: string;

  constructor(baseUrl: string, apiKey?: string) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.apiKey = apiKey;
  }

  private headers(): Record<string, string> {
    const h: Record<string, string> = { 'Content-Type': 'application/json' };
    if (this.apiKey) h['Authorization'] = `Bearer ${this.apiKey}`;
    return h;
  }

  private async request<T>(method: string, path: string, body?: unknown): Promise<T> {
    const resp = await fetch(`${this.baseUrl}${path}`, {
      method,
      headers: this.headers(),
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!resp.ok) {
      const err = await resp.json().catch(() => ({ error: resp.statusText }));
      throw new Error(`AetherDB ${method} ${path}: ${resp.status} ${(err as any).error || resp.statusText}`);
    }
    if (resp.status === 204) return {} as T;
    return resp.json() as T;
  }

  // Documents
  async createDocument(content: string, metadata: Record<string, unknown> = {}, opts?: { deduplicate?: boolean }): Promise<CreateDocumentResponse> {
    return this.request('POST', '/v1/documents', { content, metadata, deduplicate: opts?.deduplicate });
  }

  async getDocument(id: string): Promise<DocumentDto> {
    return this.request('GET', `/v1/documents/${id}`);
  }

  async updateDocument(id: string, content: string, metadata: Record<string, unknown> = {}): Promise<CreateDocumentResponse> {
    return this.request('PUT', `/v1/documents/${id}`, { content, metadata });
  }

  async deleteDocument(id: string): Promise<void> {
    await this.request('DELETE', `/v1/documents/${id}`);
  }

  async getVersions(id: string): Promise<{ document_id: string; versions: VersionDto[]; count: number }> {
    return this.request('GET', `/v1/documents/${id}/versions`);
  }

  async listDocuments(opts?: { limit?: number; offset?: number; contentContains?: string; createdAfter?: number; createdBefore?: number }): Promise<ListDocumentsResponse> {
    const params = new URLSearchParams();
    if (opts?.limit) params.set('limit', String(opts.limit));
    if (opts?.offset) params.set('offset', String(opts.offset));
    if (opts?.contentContains) params.set('content_contains', opts.contentContains);
    if (opts?.createdAfter) params.set('created_after', String(opts.createdAfter));
    if (opts?.createdBefore) params.set('created_before', String(opts.createdBefore));
    return this.request('GET', `/v1/documents/list?${params}`);
  }

  async bulkImport(documents: Array<{ content: string; metadata?: Record<string, unknown> }>, opts?: { extractEntities?: boolean }): Promise<BulkImportResponse> {
    if (opts?.extractEntities) {
      return this.request('POST', '/v1/documents/bulk', { documents, extract_entities: true });
    }
    return this.request('POST', '/v1/documents/bulk', documents);
  }

  // Search
  async search(query: string, opts?: { topK?: number; filter?: Record<string, unknown>; returnContent?: boolean }): Promise<SearchResponse> {
    return this.request('POST', '/v1/search/semantic', {
      query, top_k: opts?.topK ?? 10,
      filter: opts?.filter, return_content: opts?.returnContent,
    });
  }

  async hybridSearch(query: string, opts?: { topK?: number; graphDepth?: number; graphWeight?: number; filter?: Record<string, unknown>; returnContent?: boolean }): Promise<HybridSearchResponse> {
    return this.request('POST', '/v1/search/hybrid', {
      query, top_k: opts?.topK ?? 10,
      graph_depth: opts?.graphDepth ?? 2,
      graph_weight: opts?.graphWeight ?? 0.3,
      filter: opts?.filter, return_content: opts?.returnContent,
    });
  }

  async fulltextSearch(query: string, opts?: { topK?: number; returnContent?: boolean }): Promise<FulltextResponse> {
    return this.request('POST', '/v1/search/fulltext', {
      query, top_k: opts?.topK ?? 10, return_content: opts?.returnContent,
    });
  }

  // SQL
  async query(sql: string): Promise<{ rows: Record<string, unknown>[]; count: number }> {
    return this.request('POST', '/v1/query', { sql });
  }

  // Graph
  async getEntities(documentId: string): Promise<{ entities: EntityDto[] }> {
    return this.request('GET', `/v1/graph/entities?document_id=${documentId}`);
  }

  async getRelatedDocuments(entityName: string, depth = 2): Promise<{ documents: Array<{ document_id: string; entity_name: string; hop_count: number }> }> {
    return this.request('GET', `/v1/graph/entities/${encodeURIComponent(entityName)}/related?depth=${depth}`);
  }

  // Sync
  async syncPull(since: HlcTimestamp): Promise<SyncPullResponse> {
    return this.request('POST', '/v1/sync/pull', { since });
  }

  async syncPush(changes: Change[]): Promise<SyncPushResponse> {
    return this.request('POST', '/v1/sync/push', { changes });
  }

  // Health
  async health(): Promise<HealthResponse> {
    return this.request('GET', '/health');
  }
}

export default AetherDB;
