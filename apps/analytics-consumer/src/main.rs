#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = aetherdb_config::Config::from_env()?;
    let _guard = aetherdb_telemetry::init(&cfg.telemetry_config("analytics-consumer"))?;

    let clickhouse_url = cfg
        .clickhouse_url
        .as_deref()
        .unwrap_or("http://localhost:8123");
    let handle = aetherdb_analytics_consumer::spawn(
        &cfg.redpanda_brokers,
        clickhouse_url,
        "analytics-consumer",
    )
    .await?;

    tracing::info!("analytics-consumer ready");
    println!("analytics-consumer ready");
    tokio::signal::ctrl_c().await?;
    handle.shutdown().await;
    Ok(())
}
