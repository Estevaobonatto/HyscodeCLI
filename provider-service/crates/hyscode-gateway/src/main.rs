//! hyscode-gateway — Provider Service HTTP Gateway
//!
//! Proxy autenticado para requisições LLM com controle de quota, billing e logging.

mod auth;
mod config;
mod db;
mod error;
mod models;
mod router;
mod upstream;

use anyhow::Context;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Carrega .env se existir
    let _ = dotenvy::dotenv();

    // Inicializa logging
    fmt::Subscriber::builder()
        .with_env_filter(
            EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .json()
        .init();

    tracing::info!("Iniciando hyscode-gateway");

    let cfg = config::Config::from_env().context("Falha ao carregar configuração")?;
    let pool = db::connect(&cfg.database_url).await?;
    let redis = db::connect_redis(&cfg.redis_url).await?;

    let app = router::build_router(pool, redis, cfg);

    let listener = tokio::net::TcpListener::bind(&cfg.listen_addr).await?;
    tracing::info!(addr = %cfg.listen_addr, "Servidor escutando");

    axum::serve(listener, app).await?;

    Ok(())
}
