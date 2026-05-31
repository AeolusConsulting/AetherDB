// Contract C10 — Docker compose definition.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

#[test]
fn compose_defines_all_required_services() {
    let path = workspace_root().join("docker-compose.yml");
    assert!(path.exists(), "docker-compose.yml missing");
    let content = std::fs::read_to_string(&path).expect("read compose");
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).expect("parse yaml");
    let services = parsed.get("services").expect("services key").as_mapping().expect("mapping");
    for required in [
        "api-gateway", "embedding-worker", "analytics-consumer",
        "redpanda", "clickhouse", "prometheus", "grafana", "ollama", "sqld", "graph-worker",
    ] {
        assert!(services.get(serde_yaml::Value::String(required.into())).is_some(),
            "service '{required}' missing from docker-compose.yml");
    }
}

#[test]
fn every_service_has_healthcheck() {
    let path = workspace_root().join("docker-compose.yml");
    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_yaml::Value = serde_yaml::from_str(&content).unwrap();
    let services = parsed["services"].as_mapping().unwrap();
    for (name, svc) in services {
        let name_str = name.as_str().unwrap();
        assert!(svc.get("healthcheck").is_some(),
            "service '{name_str}' must declare a healthcheck");
    }
}

#[test]
fn prometheus_config_scrapes_all_apps() {
    let path = workspace_root().join("infra/docker/prometheus/prometheus.yml");
    assert!(path.exists(), "prometheus.yml missing");
    let content = std::fs::read_to_string(&path).unwrap();
    for app in ["api-gateway", "embedding-worker", "analytics-consumer"] {
        assert!(content.contains(app), "prometheus.yml must scrape {app}");
    }
}
