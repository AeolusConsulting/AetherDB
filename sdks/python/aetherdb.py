"""
AetherDB Python Client SDK

Usage:
    from aetherdb import AetherDB
    db = AetherDB('https://your-aetherdb.workers.dev', api_key='your-key')
    doc = db.create_document('Hello world', metadata={'source': 'test'})
    results = db.search('Hello')
"""

from dataclasses import dataclass
from typing import Any, Optional
import json
import urllib.request
import urllib.error
import urllib.parse


@dataclass
class Document:
    id: str
    content: str
    metadata: dict
    created_at: str
    updated_at: str


@dataclass
class SearchHit:
    document_id: str
    score: float
    content_preview: str
    content: Optional[str] = None
    metadata: Optional[dict] = None


@dataclass
class HybridSearchHit:
    document_id: str
    score: float
    vector_score: float
    graph_score: float
    content_preview: str
    content: Optional[str] = None
    metadata: Optional[dict] = None


@dataclass
class FulltextHit:
    document_id: str
    snippet: str
    rank: float
    content: Optional[str] = None
    metadata: Optional[dict] = None


@dataclass
class Entity:
    id: str
    name: str
    entity_type: str
    document_id: str
    created_at: str


@dataclass
class Version:
    version_id: str
    content: str
    metadata: dict
    content_hash: str
    created_at: str


@dataclass
class HlcTimestamp:
    wall_ms: int
    counter: int


@dataclass
class Change:
    document_id: str
    content: str
    metadata: dict
    content_hash: str
    hlc_ts: HlcTimestamp
    origin_node: str
    deleted: bool


@dataclass
class SyncPullResponse:
    changes: list
    server_hlc: HlcTimestamp


@dataclass
class SyncPushResponse:
    accepted: int
    rejected: int
    server_hlc: HlcTimestamp


class AetherDBError(Exception):
    def __init__(self, status: int, message: str):
        self.status = status
        self.message = message
        super().__init__(f"AetherDB error {status}: {message}")


class AetherDB:
    def __init__(self, base_url: str, api_key: Optional[str] = None):
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key

    def _headers(self) -> dict:
        h = {"Content-Type": "application/json"}
        if self.api_key:
            h["Authorization"] = f"Bearer {self.api_key}"
        return h

    def _request(self, method: str, path: str, body: Any = None) -> Any:
        url = f"{self.base_url}{path}"
        data = json.dumps(body).encode() if body else None
        req = urllib.request.Request(url, data=data, headers=self._headers(), method=method)
        try:
            with urllib.request.urlopen(req) as resp:
                if resp.status == 204:
                    return {}
                return json.loads(resp.read())
        except urllib.error.HTTPError as e:
            body_text = e.read().decode() if e.fp else ""
            try:
                err = json.loads(body_text)
                msg = err.get("error", body_text)
            except json.JSONDecodeError:
                msg = body_text
            raise AetherDBError(e.code, msg) from e

    # Documents
    def create_document(self, content: str, metadata: Optional[dict] = None, deduplicate: Optional[bool] = None) -> dict:
        body: dict = {"content": content, "metadata": metadata or {}}
        if deduplicate is not None:
            body["deduplicate"] = deduplicate
        return self._request("POST", "/v1/documents", body)

    def get_document(self, doc_id: str) -> Document:
        data = self._request("GET", f"/v1/documents/{doc_id}")
        return Document(**data)

    def update_document(self, doc_id: str, content: str, metadata: Optional[dict] = None) -> dict:
        return self._request("PUT", f"/v1/documents/{doc_id}", {"content": content, "metadata": metadata or {}})

    def delete_document(self, doc_id: str) -> None:
        self._request("DELETE", f"/v1/documents/{doc_id}")

    def get_versions(self, doc_id: str) -> list:
        data = self._request("GET", f"/v1/documents/{doc_id}/versions")
        return [Version(**v) for v in data.get("versions", [])]

    def list_documents(self, limit: int = 50, offset: int = 0, content_contains: Optional[str] = None,
                       created_after: Optional[int] = None, created_before: Optional[int] = None) -> dict:
        params = {"limit": str(limit), "offset": str(offset)}
        if content_contains:
            params["content_contains"] = content_contains
        if created_after is not None:
            params["created_after"] = str(created_after)
        if created_before is not None:
            params["created_before"] = str(created_before)
        qs = urllib.parse.urlencode(params)
        return self._request("GET", f"/v1/documents/list?{qs}")

    def bulk_import(self, documents: list, extract_entities: bool = False) -> dict:
        if extract_entities:
            return self._request("POST", "/v1/documents/bulk", {"documents": documents, "extract_entities": True})
        return self._request("POST", "/v1/documents/bulk", documents)

    # Search
    def search(self, query: str, top_k: int = 10, filter: Optional[dict] = None,
               return_content: bool = False) -> list:
        body: dict = {"query": query, "top_k": top_k, "return_content": return_content}
        if filter:
            body["filter"] = filter
        data = self._request("POST", "/v1/search/semantic", body)
        return [SearchHit(**h) for h in data.get("results", [])]

    def hybrid_search(self, query: str, top_k: int = 10, graph_depth: int = 2,
                      graph_weight: float = 0.3, filter: Optional[dict] = None,
                      return_content: bool = False) -> list:
        body: dict = {"query": query, "top_k": top_k, "graph_depth": graph_depth,
                      "graph_weight": graph_weight, "return_content": return_content}
        if filter:
            body["filter"] = filter
        data = self._request("POST", "/v1/search/hybrid", body)
        return [HybridSearchHit(**h) for h in data.get("results", [])]

    def fulltext_search(self, query: str, top_k: int = 10, return_content: bool = False) -> list:
        data = self._request("POST", "/v1/search/fulltext",
                             {"query": query, "top_k": top_k, "return_content": return_content})
        return [FulltextHit(**h) for h in data.get("results", [])]

    # SQL
    def query(self, sql: str) -> dict:
        return self._request("POST", "/v1/query", {"sql": sql})

    # Graph
    def get_entities(self, document_id: str) -> list:
        data = self._request("GET", f"/v1/graph/entities?document_id={document_id}")
        return [Entity(**e) for e in data.get("entities", [])]

    def get_related_documents(self, entity_name: str, depth: int = 2) -> list:
        data = self._request("GET", f"/v1/graph/entities/{urllib.parse.quote(entity_name)}/related?depth={depth}")
        return data.get("documents", [])

    # Sync
    def sync_pull(self, since: HlcTimestamp) -> SyncPullResponse:
        data = self._request("POST", "/v1/sync/pull", {"since": {"wall_ms": since.wall_ms, "counter": since.counter}})
        return SyncPullResponse(
            changes=data.get("changes", []),
            server_hlc=HlcTimestamp(**data.get("server_hlc", {"wall_ms": 0, "counter": 0})),
        )

    def sync_push(self, changes: list) -> SyncPushResponse:
        data = self._request("POST", "/v1/sync/push", {"changes": changes})
        return SyncPushResponse(
            accepted=data.get("accepted", 0),
            rejected=data.get("rejected", 0),
            server_hlc=HlcTimestamp(**data.get("server_hlc", {"wall_ms": 0, "counter": 0})),
        )

    # Health
    def health(self) -> dict:
        return self._request("GET", "/health")
