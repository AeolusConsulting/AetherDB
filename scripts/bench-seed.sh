#!/usr/bin/env bash
# scripts/bench-seed.sh
#
# Seeds the running AetherDB gateway with the benchmark corpus.
# Waits for all embeddings to complete before exiting.
#
# Prerequisites:
#   - Gateway running at AETHERDB_GATEWAY_BIND (default http://localhost:8080)
#   - Corpus downloaded via scripts/fetch-corpus.sh
#
# Usage: bash scripts/bench-seed.sh

set -euo pipefail

cd "$(dirname "$0")/.."

GATEWAY="${AETHERDB_GATEWAY_BIND:-http://localhost:8080}"
CORPUS="tests/corpus/msmarco_10k.json"

if [ ! -f "$CORPUS" ]; then
    echo "Corpus not found. Run: bash scripts/fetch-corpus.sh"
    exit 1
fi

TOTAL=$(wc -l < "$CORPUS")
echo "==> Seeding ${TOTAL} documents to ${GATEWAY}..."

COUNT=0
while IFS= read -r line; do
    TEXT=$(echo "$line" | python3 -c "import sys,json; print(json.load(sys.stdin)['text'])")
    curl -s -X POST "${GATEWAY}/v1/documents" \
        -H "Content-Type: application/json" \
        -d "{\"content\": $(echo "$TEXT" | python3 -c 'import sys,json; print(json.dumps(sys.stdin.read().strip()))')}" \
        > /dev/null
    COUNT=$((COUNT + 1))
    if [ $((COUNT % 500)) -eq 0 ]; then
        echo "  seeded ${COUNT}/${TOTAL}..."
    fi
done < "$CORPUS"

echo "==> Seeded ${COUNT} documents. Waiting for embeddings..."

# Poll until pending embeddings drops to 0 or timeout.
TIMEOUT=300
ELAPSED=0
while [ "$ELAPSED" -lt "$TIMEOUT" ]; do
    PENDING=$(curl -s -X POST "${GATEWAY}/v1/search/semantic" \
        -H "Content-Type: application/json" \
        -d '{"query":"test","top_k":1}' \
        -o /dev/null -w '' -D - 2>/dev/null \
        | grep -i "x-pending-embeddings" \
        | awk '{print $2}' \
        | tr -d '\r' || echo "unknown")
    if [ "$PENDING" = "0" ]; then
        echo "==> All embeddings complete."
        break
    fi
    echo "  pending: ${PENDING}, waiting..."
    sleep 5
    ELAPSED=$((ELAPSED + 5))
done

echo "==> Seeding complete."
