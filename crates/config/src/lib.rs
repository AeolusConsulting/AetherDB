use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub libsql_url: String,
    pub libsql_auth_token: Option<String>,
    pub embed_dim: u16,
    pub embed_provider: EmbedProviderKind,
    pub embed_base_url: String,
    pub embed_model: String,
    pub embed_api_key: Option<String>,
    pub redpanda_brokers: String,
    pub clickhouse_url: Option<String>,
    pub gateway_bind: SocketAddr,
    pub otlp_endpoint: Option<String>,
    pub prometheus_bind: SocketAddr,
    pub log_level: String,
    pub minimal_mode: bool,
    pub export_dir: PathBuf,
    pub export_cron: String,
    pub replica_local_path: Option<String>,
    pub sync_interval_secs: u64,
    pub storage_role: StorageRole,
    pub llm_provider: LlmProviderKind,
    pub llm_base_url: String,
    pub llm_model: String,
    pub llm_api_key: Option<String>,
    pub graph_enabled: bool,
    pub hnsw_enabled: bool,
    pub hnsw_ef_construction: usize,
    pub hnsw_m: usize,
    pub hnsw_ef_search: usize,
    pub hnsw_rebuild_interval_secs: u64,
    pub federation_enabled: bool,
    pub federation_hub_url: Option<String>,
    pub federation_role: FederationRole,
    pub federation_sync_interval_secs: u64,
    pub federation_node_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedProviderKind {
    Ollama,
    OpenAi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageRole {
    Primary,
    Replica,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FederationRole {
    Hub,
    Spoke,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmProviderKind {
    Ollama,
    OpenAi,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required environment variable: {var}")]
    Missing { var: String },
    #[error("invalid value for {var}: {reason}")]
    Invalid { var: String, reason: String },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let libsql_url = required("AETHERDB_LIBSQL_URL")?;
        let libsql_auth_token = optional("AETHERDB_LIBSQL_AUTH_TOKEN");
        let redpanda_brokers = required("AETHERDB_REDPANDA_BROKERS")?;
        let clickhouse_url = optional("AETHERDB_CLICKHOUSE_URL");

        let embed_dim = optional("AETHERDB_EMBED_DIM")
            .map(|s| parse_embed_dim(&s))
            .transpose()?
            .unwrap_or(768);

        let embed_provider = optional("AETHERDB_EMBED_PROVIDER")
            .map(|s| parse_provider(&s))
            .transpose()?
            .unwrap_or(EmbedProviderKind::Ollama);

        let embed_base_url = optional("AETHERDB_EMBED_BASE_URL")
            .unwrap_or_else(|| "http://localhost:11434/v1".into());

        let embed_model =
            optional("AETHERDB_EMBED_MODEL").unwrap_or_else(|| "nomic-embed-text".into());

        let embed_api_key = optional("AETHERDB_EMBED_API_KEY");

        if embed_provider == EmbedProviderKind::OpenAi && embed_api_key.is_none() {
            return Err(ConfigError::Invalid {
                var: "AETHERDB_EMBED_API_KEY".into(),
                reason: "required when AETHERDB_EMBED_PROVIDER=openai".into(),
            });
        }

        let gateway_bind = optional("AETHERDB_GATEWAY_BIND")
            .map(|s| parse_socket_addr("AETHERDB_GATEWAY_BIND", &s))
            .transpose()?
            .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 8080)));

        let prometheus_bind = optional("AETHERDB_PROMETHEUS_BIND")
            .map(|s| parse_socket_addr("AETHERDB_PROMETHEUS_BIND", &s))
            .transpose()?
            .unwrap_or_else(|| SocketAddr::from(([0, 0, 0, 0], 9090)));

        let otlp_endpoint = optional("AETHERDB_OTLP_ENDPOINT");

        let log_level = optional("AETHERDB_LOG_LEVEL").unwrap_or_else(|| "info".into());

        let minimal_mode = optional("AETHERDB_MINIMAL_MODE")
            .map(|s| parse_bool("AETHERDB_MINIMAL_MODE", &s))
            .transpose()?
            .unwrap_or(false);

        let export_dir = optional("AETHERDB_EXPORT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./exports/"));

        let export_cron = optional("AETHERDB_EXPORT_CRON").unwrap_or_else(|| "0 * * * *".into());

        let replica_local_path = optional("AETHERDB_REPLICA_LOCAL_PATH");

        let sync_interval_secs = optional("AETHERDB_SYNC_INTERVAL_SECS")
            .map(|s| {
                s.parse::<u64>().map_err(|_| ConfigError::Invalid {
                    var: "AETHERDB_SYNC_INTERVAL_SECS".into(),
                    reason: format!("not a valid integer: {s}"),
                })
            })
            .transpose()?
            .unwrap_or(1);

        let storage_role = optional("AETHERDB_STORAGE_ROLE")
            .map(|s| parse_storage_role("AETHERDB_STORAGE_ROLE", &s))
            .transpose()?
            .unwrap_or_else(|| {
                if replica_local_path.is_some() && !libsql_url.starts_with("file:") {
                    StorageRole::Replica
                } else {
                    StorageRole::Primary
                }
            });

        if storage_role == StorageRole::Replica {
            if replica_local_path.is_none() {
                return Err(ConfigError::Invalid {
                    var: "AETHERDB_REPLICA_LOCAL_PATH".into(),
                    reason: "required when storage role is replica".into(),
                });
            }
            if libsql_url.starts_with("file:") {
                return Err(ConfigError::Invalid {
                    var: "AETHERDB_LIBSQL_URL".into(),
                    reason: "replica mode requires a remote libsql URL, not file:".into(),
                });
            }
        }

        let llm_provider = optional("AETHERDB_LLM_PROVIDER")
            .map(|s| parse_llm_provider("AETHERDB_LLM_PROVIDER", &s))
            .transpose()?
            .unwrap_or(LlmProviderKind::Ollama);

        let llm_base_url =
            optional("AETHERDB_LLM_BASE_URL").unwrap_or_else(|| "http://localhost:11434/v1".into());

        let llm_model = optional("AETHERDB_LLM_MODEL").unwrap_or_else(|| "llama3.2".into());

        let llm_api_key = optional("AETHERDB_LLM_API_KEY");

        if llm_provider == LlmProviderKind::OpenAi && llm_api_key.is_none() {
            return Err(ConfigError::Invalid {
                var: "AETHERDB_LLM_API_KEY".into(),
                reason: "required when AETHERDB_LLM_PROVIDER=openai".into(),
            });
        }

        let graph_enabled = optional("AETHERDB_GRAPH_ENABLED")
            .map(|s| parse_bool("AETHERDB_GRAPH_ENABLED", &s))
            .transpose()?
            .unwrap_or(false);

        let hnsw_enabled = optional("AETHERDB_HNSW_ENABLED")
            .map(|s| parse_bool("AETHERDB_HNSW_ENABLED", &s))
            .transpose()?
            .unwrap_or(false);

        let hnsw_ef_construction = optional("AETHERDB_HNSW_EF_CONSTRUCTION")
            .map(|s| {
                s.parse::<usize>().map_err(|_| ConfigError::Invalid {
                    var: "AETHERDB_HNSW_EF_CONSTRUCTION".into(),
                    reason: format!("not a valid integer: {s}"),
                })
            })
            .transpose()?
            .unwrap_or(100);

        let hnsw_m = optional("AETHERDB_HNSW_M")
            .map(|s| {
                s.parse::<usize>().map_err(|_| ConfigError::Invalid {
                    var: "AETHERDB_HNSW_M".into(),
                    reason: format!("not a valid integer: {s}"),
                })
            })
            .transpose()?
            .unwrap_or(16);

        let hnsw_ef_search = optional("AETHERDB_HNSW_EF_SEARCH")
            .map(|s| {
                s.parse::<usize>().map_err(|_| ConfigError::Invalid {
                    var: "AETHERDB_HNSW_EF_SEARCH".into(),
                    reason: format!("not a valid integer: {s}"),
                })
            })
            .transpose()?
            .unwrap_or(50);

        let hnsw_rebuild_interval_secs = optional("AETHERDB_HNSW_REBUILD_INTERVAL")
            .map(|s| {
                s.parse::<u64>().map_err(|_| ConfigError::Invalid {
                    var: "AETHERDB_HNSW_REBUILD_INTERVAL".into(),
                    reason: format!("not a valid integer: {s}"),
                })
            })
            .transpose()?
            .unwrap_or(300);

        let federation_enabled = optional("AETHERDB_FEDERATION_ENABLED")
            .map(|s| parse_bool("AETHERDB_FEDERATION_ENABLED", &s))
            .transpose()?
            .unwrap_or(false);

        let federation_hub_url = optional("AETHERDB_FEDERATION_HUB_URL");

        let federation_role = optional("AETHERDB_FEDERATION_ROLE")
            .map(|s| parse_federation_role("AETHERDB_FEDERATION_ROLE", &s))
            .transpose()?
            .unwrap_or(FederationRole::Hub);

        let federation_sync_interval_secs = optional("AETHERDB_FEDERATION_SYNC_INTERVAL_SECS")
            .map(|s| {
                s.parse::<u64>().map_err(|_| ConfigError::Invalid {
                    var: "AETHERDB_FEDERATION_SYNC_INTERVAL_SECS".into(),
                    reason: format!("not a valid integer: {s}"),
                })
            })
            .transpose()?
            .unwrap_or(30);

        let federation_node_id = optional("AETHERDB_FEDERATION_NODE_ID");

        Ok(Config {
            libsql_url,
            libsql_auth_token,
            embed_dim,
            embed_provider,
            embed_base_url,
            embed_model,
            embed_api_key,
            redpanda_brokers,
            clickhouse_url,
            gateway_bind,
            otlp_endpoint,
            prometheus_bind,
            log_level,
            minimal_mode,
            export_dir,
            export_cron,
            replica_local_path,
            sync_interval_secs,
            storage_role,
            llm_provider,
            llm_base_url,
            llm_model,
            llm_api_key,
            graph_enabled,
            hnsw_enabled,
            hnsw_ef_construction,
            hnsw_m,
            hnsw_ef_search,
            hnsw_rebuild_interval_secs,
            federation_enabled,
            federation_hub_url,
            federation_role,
            federation_sync_interval_secs,
            federation_node_id,
        })
    }

    pub fn telemetry_config(&self, service_name: &str) -> aetherdb_telemetry::TelemetryConfig {
        aetherdb_telemetry::TelemetryConfig {
            service_name: service_name.into(),
            log_level: self.log_level.clone(),
            otlp_endpoint: self.otlp_endpoint.clone(),
            prometheus_bind: self.prometheus_bind,
        }
    }
}

fn required(var: &str) -> Result<String, ConfigError> {
    std::env::var(var).map_err(|_| ConfigError::Missing { var: var.into() })
}

fn optional(var: &str) -> Option<String> {
    std::env::var(var).ok().filter(|s| !s.is_empty())
}

fn parse_embed_dim(s: &str) -> Result<u16, ConfigError> {
    let val: u32 = s.parse().map_err(|_| ConfigError::Invalid {
        var: "AETHERDB_EMBED_DIM".into(),
        reason: format!("not a valid integer: {s}"),
    })?;
    if !(1..=65_536).contains(&val) {
        return Err(ConfigError::Invalid {
            var: "AETHERDB_EMBED_DIM".into(),
            reason: format!("must be in 1..=65536, got {val}"),
        });
    }
    Ok(val as u16)
}

fn parse_provider(s: &str) -> Result<EmbedProviderKind, ConfigError> {
    match s.to_lowercase().as_str() {
        "ollama" => Ok(EmbedProviderKind::Ollama),
        "openai" => Ok(EmbedProviderKind::OpenAi),
        _ => Err(ConfigError::Invalid {
            var: "AETHERDB_EMBED_PROVIDER".into(),
            reason: format!("must be 'ollama' or 'openai', got '{s}'"),
        }),
    }
}

fn parse_socket_addr(var: &str, s: &str) -> Result<SocketAddr, ConfigError> {
    s.parse().map_err(|e| ConfigError::Invalid {
        var: var.into(),
        reason: format!("invalid socket address: {e}"),
    })
}

fn parse_bool(var: &str, s: &str) -> Result<bool, ConfigError> {
    match s.to_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(true),
        "false" | "0" | "no" => Ok(false),
        _ => Err(ConfigError::Invalid {
            var: var.into(),
            reason: format!("expected boolean, got '{s}'"),
        }),
    }
}

fn parse_llm_provider(var: &str, s: &str) -> Result<LlmProviderKind, ConfigError> {
    match s.to_lowercase().as_str() {
        "ollama" => Ok(LlmProviderKind::Ollama),
        "openai" => Ok(LlmProviderKind::OpenAi),
        _ => Err(ConfigError::Invalid {
            var: var.into(),
            reason: format!("must be 'ollama' or 'openai', got '{s}'"),
        }),
    }
}

fn parse_storage_role(var: &str, s: &str) -> Result<StorageRole, ConfigError> {
    match s.to_lowercase().as_str() {
        "primary" => Ok(StorageRole::Primary),
        "replica" => Ok(StorageRole::Replica),
        _ => Err(ConfigError::Invalid {
            var: var.into(),
            reason: format!("must be 'primary' or 'replica', got '{s}'"),
        }),
    }
}

fn parse_federation_role(var: &str, s: &str) -> Result<FederationRole, ConfigError> {
    match s.to_lowercase().as_str() {
        "hub" => Ok(FederationRole::Hub),
        "spoke" => Ok(FederationRole::Spoke),
        _ => Err(ConfigError::Invalid {
            var: var.into(),
            reason: format!("must be 'hub' or 'spoke', got '{s}'"),
        }),
    }
}
