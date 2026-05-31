#!/usr/bin/env bash
# scripts/verify.sh
#
# Runs the full contract verification suite. CI runs this on every PR.
# Returns non-zero if any step fails.

set -euo pipefail

cd "$(dirname "$0")/.."

step() {
    echo
    echo "============================================================"
    echo "== $*"
    echo "============================================================"
}

step "1/6  cargo fmt --check"
cargo fmt --all -- --check

step "2/6  cargo clippy --all-targets -- -D warnings"
cargo clippy --workspace --all-targets -- -D warnings

step "3/6  cargo test --workspace --lib"
cargo test --workspace --lib

step "4/6  scripts/smoke-libsql-vector.sh"
bash scripts/smoke-libsql-vector.sh

step "5/6  cargo test --workspace --tests (non-ignored)"
cargo test --workspace --tests

step "6/6  scripts/check-no-stubs.sh"
bash scripts/check-no-stubs.sh

echo
echo "============================================================"
echo "All verification steps passed."
echo "============================================================"
