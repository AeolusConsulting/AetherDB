#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = aetherdb_config::Config::from_env()?;
    let _guard = aetherdb_telemetry::init(&cfg.telemetry_config("duckdb-exporter"))?;

    tracing::info!("duckdb-exporter ready, cron: {}", cfg.export_cron);
    println!("duckdb-exporter ready");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("shutdown signal received");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(3600)) => {
                tracing::info!("running scheduled export");
                if let Err(e) = aetherdb_duckdb_exporter::run_once(&cfg.libsql_url, &cfg.export_dir).await {
                    tracing::error!("export failed: {e}");
                }
            }
        }
    }

    Ok(())
}
