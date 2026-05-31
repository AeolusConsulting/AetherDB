// Contract C1.3 — required files at repo root

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

#[test]
fn env_example_exists_and_lists_all_vars() {
    let root = workspace_root();
    let path = root.join(".env.example");
    assert!(path.exists(), ".env.example missing");
    let content = std::fs::read_to_string(&path).expect("read .env.example");

    let required_vars = [
        "AETHERDB_LIBSQL_URL",
        "AETHERDB_EMBED_DIM",
        "AETHERDB_EMBED_PROVIDER",
        "AETHERDB_EMBED_BASE_URL",
        "AETHERDB_EMBED_MODEL",
        "AETHERDB_REDPANDA_BROKERS",
        "AETHERDB_CLICKHOUSE_URL",
        "AETHERDB_GATEWAY_BIND",
        "AETHERDB_PROMETHEUS_BIND",
        "AETHERDB_LOG_LEVEL",
        "AETHERDB_MINIMAL_MODE",
        "AETHERDB_EXPORT_DIR",
        "AETHERDB_EXPORT_CRON",
    ];
    for v in required_vars {
        assert!(content.contains(v), ".env.example is missing variable {}", v);
    }
}

#[test]
fn makefile_has_required_targets() {
    let root = workspace_root();
    let path = root.join("Makefile");
    assert!(path.exists(), "Makefile missing");
    let content = std::fs::read_to_string(&path).expect("read Makefile");
    for target in ["build:", "test:", "lint:", "fmt:", "smoke:", "bench-seed:", "bench-search:"] {
        assert!(content.contains(target), "Makefile is missing target: {}", target);
    }
}

#[test]
fn docker_compose_exists() {
    let root = workspace_root();
    assert!(root.join("docker-compose.yml").exists(), "docker-compose.yml missing at repo root");
}

#[test]
fn github_actions_workflow_exists() {
    let root = workspace_root();
    let path = root.join(".github/workflows/ci.yml");
    assert!(path.exists(), ".github/workflows/ci.yml missing");
    let content = std::fs::read_to_string(&path).expect("read ci.yml");
    for required in ["cargo test", "cargo clippy", "cargo fmt", "smoke-libsql-vector.sh"] {
        assert!(content.contains(required), "CI workflow is missing: {}", required);
    }
}
