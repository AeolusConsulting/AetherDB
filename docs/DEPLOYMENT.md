# AetherDB Deployment Guide

## Quick Start (Docker Compose)

```bash
cp .env.example .env
docker compose up -d
```

This starts: api-gateway, embedding-worker, graph-worker, analytics-consumer, sqld, redpanda, clickhouse, ollama, prometheus, grafana.

### Access Points
- API Gateway: http://localhost:8080
- Grafana: http://localhost:3000
- Prometheus: http://localhost:9091
- ClickHouse: http://localhost:8123

## Kubernetes (Helm)

```bash
helm install aetherdb ./helm/aetherdb/

# With GraphRAG and HNSW enabled:
helm install aetherdb ./helm/aetherdb/ \
  --set graphWorker.enabled=true \
  --set graph.enabled=true \
  --set hnsw.enabled=true

# With Ingress:
helm install aetherdb ./helm/aetherdb/ \
  --set apiGateway.ingress.enabled=true \
  --set apiGateway.ingress.host=api.example.com
```

## Distributed Mode (libSQL Replicas)

Multiple gateway instances each with an embedded replica syncing from sqld:

```bash
docker compose -f docker-compose.yml -f docker-compose.distributed.yml up
```

Environment:
```
AETHERDB_LIBSQL_URL=http://sqld:8080
AETHERDB_REPLICA_LOCAL_PATH=/data/replica.db
AETHERDB_STORAGE_ROLE=replica
```

## Federation (Multi-Region)

Hub-and-spoke topology for multi-region sync:

```bash
docker compose -f docker-compose.yml -f docker-compose.federation.yml up
```

Environment (spoke):
```
AETHERDB_FEDERATION_ENABLED=true
AETHERDB_FEDERATION_ROLE=spoke
AETHERDB_FEDERATION_HUB_URL=http://hub-gateway:8080
AETHERDB_FEDERATION_SYNC_INTERVAL_SECS=30
```

## Authentication

Set `AETHERDB_API_KEYS` to a comma-separated list of API keys:
```
AETHERDB_API_KEYS=key1,key2,key3
```

Clients authenticate with `Authorization: Bearer <key>`. If `AETHERDB_API_KEYS` is empty, auth is disabled (dev mode).

## Environment Variable Reference

### Required
| Variable | Description |
|----------|-------------|
| `AETHERDB_LIBSQL_URL` | Database URL (`file:./aether.db` or `http://sqld:8080`) |
| `AETHERDB_REDPANDA_BROKERS` | Kafka brokers (`localhost:9092`) |

### Optional
| Variable | Default | Description |
|----------|---------|-------------|
| `AETHERDB_GATEWAY_BIND` | `0.0.0.0:8080` | HTTP listen address |
| `AETHERDB_EMBED_DIM` | `768` | Embedding dimension |
| `AETHERDB_EMBED_PROVIDER` | `ollama` | `ollama` or `openai` |
| `AETHERDB_EMBED_MODEL` | `nomic-embed-text` | Model name |
| `AETHERDB_MINIMAL_MODE` | `false` | Skip event publishing |
| `AETHERDB_GRAPH_ENABLED` | `false` | Enable GraphRAG |
| `AETHERDB_HNSW_ENABLED` | `false` | Enable in-memory HNSW |
| `AETHERDB_FEDERATION_ENABLED` | `false` | Enable federation |
| `AETHERDB_API_KEYS` | (empty) | Auth keys (empty = disabled) |
| `AETHERDB_LOG_LEVEL` | `info` | Tracing filter |

See `.env.example` for the complete list.

## Monitoring

Prometheus scrapes all services on port 9090. Key metrics:
- `http_requests_total{method,path,status}` — request rate
- `http_request_duration_seconds` — latency histogram
- `vector_search_duration_seconds` — search performance
- `kafka_consumer_lag{topic,partition}` — event processing lag

Grafana dashboard at `infra/docker/grafana/dashboards/aetherdb.json`.
