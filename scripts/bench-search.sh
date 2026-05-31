#!/usr/bin/env bash
# scripts/bench-search.sh
#
# Runs benchmark search queries against a seeded AetherDB gateway.
# Reports p50/p95/p99 latency and appends results to docs/BENCHMARKS.md.
#
# Prerequisites:
#   - Gateway running and seeded (via bench-seed.sh)
#
# Usage: bash scripts/bench-search.sh

set -euo pipefail

cd "$(dirname "$0")/.."

GATEWAY="${AETHERDB_GATEWAY_BIND:-http://localhost:8080}"
NUM_QUERIES=1000

echo "==> Running ${NUM_QUERIES} search queries against ${GATEWAY}..."

LATENCIES=()
for i in $(seq 1 "$NUM_QUERIES"); do
    START=$(date +%s%N)
    curl -s -X POST "${GATEWAY}/v1/search/semantic" \
        -H "Content-Type: application/json" \
        -d '{"query":"information retrieval search","top_k":10}' \
        > /dev/null
    END=$(date +%s%N)
    MS=$(( (END - START) / 1000000 ))
    LATENCIES+=("$MS")

    if [ $((i % 100)) -eq 0 ]; then
        echo "  completed ${i}/${NUM_QUERIES}..."
    fi
done

# Sort latencies for percentile calculation.
SORTED=($(printf '%s\n' "${LATENCIES[@]}" | sort -n))
LEN=${#SORTED[@]}

P50_IDX=$(( LEN * 50 / 100 ))
P95_IDX=$(( LEN * 95 / 100 ))
P99_IDX=$(( LEN * 99 / 100 ))

P50=${SORTED[$P50_IDX]}
P95=${SORTED[$P95_IDX]}
P99=${SORTED[$P99_IDX]}

echo ""
echo "==> Results (${NUM_QUERIES} queries):"
echo "  p50:  ${P50}ms"
echo "  p95:  ${P95}ms"
echo "  p99:  ${P99}ms"
echo "  recall@10: TBD (requires qrels)"

# Append to BENCHMARKS.md
DATE=$(date -u +%Y-%m-%d)
HARDWARE=$(uname -m)
echo "| ${DATE} | ${HARDWARE} | ${P50}ms | ${P95}ms | ${P99}ms | TBD | bench-search run |" >> docs/BENCHMARKS.md
echo "==> Appended results to docs/BENCHMARKS.md"
