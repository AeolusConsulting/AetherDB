.PHONY: build test lint fmt smoke bench-seed bench-search

build:
	cargo build --workspace

test:
	cargo test --workspace

lint:
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all -- --check

smoke:
	bash scripts/smoke-libsql-vector.sh

bench-seed:
	bash scripts/bench-seed.sh

bench-search:
	bash scripts/bench-search.sh
