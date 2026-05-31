#!/usr/bin/env bash
# scripts/smoke-libsql-vector.sh
#
# Contract C1.4 — proves that the pinned libSQL crate version actually
# supports vector_top_k + libsql_vector_idx, and that the query plan
# consults the index rather than full-scanning.
#
# This script is the GATE for Task 4. If it fails, do not implement
# crates/vector — the libSQL version is unsuitable.

set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> Building smoke example..."
cargo build --release --example libsql_smoke

echo "==> Running smoke example..."
output=$(cargo run --release --example libsql_smoke 2>&1)
echo "$output"

# Assert plan contains VIRTUAL TABLE INDEX (the DiskANN vector index marker).
if ! echo "$output" | grep -q "VIRTUAL TABLE INDEX"; then
    echo
    echo "FAIL: EXPLAIN QUERY PLAN does not contain 'VIRTUAL TABLE INDEX'."
    echo "The vector index is not being used — vector_top_k is missing or"
    echo "the JOIN shape is wrong."
    exit 1
fi

# Assert no full scan of document_embeddings.
if echo "$output" | grep -q "SCAN document_embeddings"; then
    echo
    echo "FAIL: EXPLAIN QUERY PLAN shows a full SCAN of document_embeddings."
    echo "The query is not consulting the index."
    exit 1
fi

echo
echo "OK: libSQL vector search end-to-end check passed."
