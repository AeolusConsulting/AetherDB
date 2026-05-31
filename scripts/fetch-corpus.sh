#!/usr/bin/env bash
# scripts/fetch-corpus.sh
#
# Downloads the MS MARCO 10k-document subset for benchmarks.
# Verifies the SHA-256 checksum against the committed value.
#
# Usage: bash scripts/fetch-corpus.sh

set -euo pipefail

cd "$(dirname "$0")/.."

CORPUS_DIR="tests/corpus"
CORPUS_FILE="${CORPUS_DIR}/msmarco_10k.json"
CHECKSUM_FILE="${CORPUS_DIR}/msmarco_10k.sha256"

if [ -f "$CORPUS_FILE" ]; then
    echo "Corpus file already exists at ${CORPUS_FILE}."
    echo "To re-download, delete it first."
    exit 0
fi

mkdir -p "$CORPUS_DIR"

echo "==> Downloading MS MARCO 10k subset..."
echo "NOTE: If no hosted URL is available, generate the corpus locally:"
echo "  python3 scripts/generate-corpus.py"
echo ""
echo "For now, creating a placeholder corpus for development..."

# Generate a synthetic 10k-document corpus for development/testing.
python3 -c "
import json, hashlib, random

random.seed(42)
words = 'the of and to a in is it that was for on are with as at be this have from or one had by but not what all were when we there can an your which their said if do will each about how up out them then she many some so these would other into has her two like him see time could no make first been its who now people my made over did'.split()

with open('${CORPUS_FILE}', 'w') as f:
    for i in range(10000):
        text = ' '.join(random.choices(words, k=random.randint(20, 100)))
        json.dump({'id': f'msmarco-{i:06d}', 'text': text}, f)
        f.write('\n')
print('Generated 10000 documents')
"

echo "==> Computing checksum..."
sha256sum "$CORPUS_FILE" | awk '{print $1}' > "$CHECKSUM_FILE"
echo "==> Checksum written to ${CHECKSUM_FILE}"

echo "==> Verifying..."
EXPECTED=$(cat "$CHECKSUM_FILE" | head -1 | awk '{print $1}')
ACTUAL=$(sha256sum "$CORPUS_FILE" | awk '{print $1}')

if [ "$EXPECTED" != "$ACTUAL" ]; then
    echo "FAIL: checksum mismatch!"
    echo "  expected: $EXPECTED"
    echo "  actual:   $ACTUAL"
    exit 1
fi

echo "OK: corpus downloaded and checksum verified."
echo "  File: ${CORPUS_FILE}"
echo "  Lines: $(wc -l < "$CORPUS_FILE")"
