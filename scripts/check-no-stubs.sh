#!/usr/bin/env bash
# scripts/check-no-stubs.sh
#
# Greps production source (crates/ and apps/) for forbidden stub patterns.
# Exits non-zero on any match. Tests, examples, benches, and the
# tests/fixtures/ directory are exempt.
#
# This is the CI-side enforcement of CONTRACTS.md § "Stub detector".

set -euo pipefail

cd "$(dirname "$0")/.."

# Patterns that indicate a stub. Each is a regex passed to ripgrep.
PATTERNS=(
    '\btodo!\s*\('
    '\bunimplemented!\s*\('
    '^\s*return Default::default\(\);\s*$'
    '^\s*Ok\(Default::default\(\)\)\s*$'
    '^\s*return Vec::new\(\);\s*$'
    '^\s*return vec!\[\];\s*$'
)

# Directories to scan.
SCAN_DIRS=("crates" "apps")

# Files / paths to exclude.
EXCLUDES=(
    '--glob' '!**/tests/**'
    '--glob' '!**/benches/**'
    '--glob' '!**/examples/**'
    '--glob' '!**/test_harness*'
    '--glob' '!**/mock*'
)

found_any=0
for pat in "${PATTERNS[@]}"; do
    echo "==> Checking for pattern: $pat"
    if rg --type rust "${EXCLUDES[@]}" -n -e "$pat" "${SCAN_DIRS[@]}" 2>/dev/null; then
        found_any=1
    fi
done

if [ "$found_any" -ne 0 ]; then
    echo
    echo "FAIL: stub patterns found in production source. Implement the function or"
    echo "explicitly mark it as a test/example."
    exit 1
fi

echo "OK: no stub patterns in production source."
