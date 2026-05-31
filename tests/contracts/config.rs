// Contract: Configuration parsing.
//
// These tests must FAIL until crates/config exposes the contract API.

use aetherdb_config::{Config, ConfigError};
use std::collections::HashMap;

/// Set env vars from a HashMap for the duration of a test, restoring previous values on drop.
struct EnvGuard {
    previous: HashMap<String, Option<String>>,
}

impl EnvGuard {
    fn new(vars: &[(&str, &str)]) -> Self {
        let mut previous = HashMap::new();
        for (k, v) in vars {
            previous.insert(k.to_string(), std::env::var(k).ok());
            // SAFETY: env mutation in tests; we restore in Drop. Tests must not run in parallel
            // with each other when they touch env; mark them #[serial] if needed.
            unsafe { std::env::set_var(k, v) };
        }
        Self { previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, prev) in &self.previous {
            match prev {
                Some(v) => unsafe { std::env::set_var(k, v) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
    }
}

#[test]
fn missing_required_libsql_url_errors() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
    ]);
    unsafe { std::env::remove_var("AETHERDB_LIBSQL_URL") };
    match Config::from_env() {
        Err(ConfigError::Missing { var }) => assert_eq!(var, "AETHERDB_LIBSQL_URL"),
        other => panic!("expected Missing{{AETHERDB_LIBSQL_URL}}, got {:?}", other),
    }
}

#[test]
fn missing_required_redpanda_brokers_errors() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "file::memory:"),
    ]);
    unsafe { std::env::remove_var("AETHERDB_REDPANDA_BROKERS") };
    match Config::from_env() {
        Err(ConfigError::Missing { var }) => assert_eq!(var, "AETHERDB_REDPANDA_BROKERS"),
        other => panic!("expected Missing{{AETHERDB_REDPANDA_BROKERS}}, got {:?}", other),
    }
}

#[test]
fn embed_dim_above_limit_errors() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "file::memory:"),
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
        ("AETHERDB_EMBED_DIM", "70000"),
    ]);
    match Config::from_env() {
        Err(ConfigError::Invalid { .. }) => {},
        other => panic!("expected Invalid for embed_dim=70000, got {:?}", other),
    }
}

#[test]
fn openai_provider_without_api_key_errors() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "file::memory:"),
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
        ("AETHERDB_EMBED_PROVIDER", "openai"),
    ]);
    unsafe { std::env::remove_var("AETHERDB_EMBED_API_KEY") };
    match Config::from_env() {
        Err(ConfigError::Invalid { .. }) => {},
        other => panic!("expected Invalid for openai without api_key, got {:?}", other),
    }
}

#[test]
fn defaults_applied_when_unset() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "file::memory:"),
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
    ]);
    let cfg = Config::from_env().expect("config with only required vars");
    assert_eq!(cfg.embed_dim, 768);
    assert_eq!(cfg.embed_model, "nomic-embed-text");
    assert_eq!(cfg.log_level, "info");
    assert!(!cfg.minimal_mode);
    assert_eq!(cfg.storage_role, aetherdb_config::StorageRole::Primary);
    assert!(cfg.replica_local_path.is_none());
    assert_eq!(cfg.sync_interval_secs, 1);
}

#[test]
fn replica_mode_requires_local_path() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "http://sqld:8080"),
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
        ("AETHERDB_STORAGE_ROLE", "replica"),
    ]);
    unsafe { std::env::remove_var("AETHERDB_REPLICA_LOCAL_PATH") };
    match Config::from_env() {
        Err(ConfigError::Invalid { .. }) => {}
        other => panic!("expected Invalid for replica without local path, got {:?}", other),
    }
}

#[test]
fn replica_mode_incompatible_with_file_url() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "file:./aether.db"),
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
        ("AETHERDB_STORAGE_ROLE", "replica"),
        ("AETHERDB_REPLICA_LOCAL_PATH", "./replica.db"),
    ]);
    match Config::from_env() {
        Err(ConfigError::Invalid { .. }) => {}
        other => panic!("expected Invalid for file: URL with replica mode, got {:?}", other),
    }
}

#[test]
fn auto_detect_replica_from_remote_url_and_local_path() {
    let _g = EnvGuard::new(&[
        ("AETHERDB_LIBSQL_URL", "http://sqld:8080"),
        ("AETHERDB_REDPANDA_BROKERS", "localhost:9092"),
        ("AETHERDB_REPLICA_LOCAL_PATH", "./replica.db"),
    ]);
    unsafe { std::env::remove_var("AETHERDB_STORAGE_ROLE") };
    let cfg = Config::from_env().expect("config with remote url + local path");
    assert_eq!(cfg.storage_role, aetherdb_config::StorageRole::Replica);
}
