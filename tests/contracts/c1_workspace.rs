// Contract C1.1 — workspace structure
//
// These tests are filesystem-level checks. They run from the workspace root
// (assumed to be `cargo test`'s working directory minus this crate's path).

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    // tests/contracts/c1_workspace.rs → up two levels
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn workspace_cargo_toml_exists() {
    let root = workspace_root();
    let cargo_toml = root.join("Cargo.toml");
    assert!(cargo_toml.exists(), "workspace Cargo.toml missing at {}", cargo_toml.display());
}

#[test]
fn workspace_has_resolver_3() {
    let root = workspace_root();
    let content = std::fs::read_to_string(root.join("Cargo.toml")).expect("read workspace Cargo.toml");
    assert!(content.contains(r#"resolver = "3""#), "workspace Cargo.toml must declare resolver = \"3\"");
}

#[test]
fn workspace_lints_deny_unwrap() {
    let root = workspace_root();
    let content = std::fs::read_to_string(root.join("Cargo.toml")).expect("read workspace Cargo.toml");
    assert!(
        content.contains("unwrap_used") && content.contains(r#""deny""#),
        "workspace Cargo.toml must deny clippy::unwrap_used at workspace level"
    );
}

#[test]
fn all_required_members_exist() {
    let root = workspace_root();
    let required = [
        "apps/api-gateway",
        "apps/embedding-worker",
        "apps/analytics-consumer",
        "apps/duckdb-exporter",
        "crates/domain",
        "crates/events",
        "crates/storage",
        "crates/vector",
        "crates/telemetry",
        "crates/config",
        "crates/messaging",
    ];
    for member in required {
        let path = root.join(member);
        assert!(path.exists(), "missing workspace member: {}", member);
        assert!(path.join("Cargo.toml").exists(), "missing Cargo.toml in {}", member);
    }
}

#[test]
fn rust_toolchain_pinned_to_1_85() {
    let root = workspace_root();
    let path = root.join("rust-toolchain.toml");
    assert!(path.exists(), "rust-toolchain.toml missing at repo root");
    let content = std::fs::read_to_string(&path).expect("read rust-toolchain.toml");
    assert!(content.contains(r#"channel = "1.85""#), "rust-toolchain.toml must pin channel = \"1.85\"");
}
