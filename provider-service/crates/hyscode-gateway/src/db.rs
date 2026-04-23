//! Conexão com PostgreSQL e Redis.

use anyhow::{Context, Result};
use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = sqlx::PgPool::connect(database_url)
        .await
        .context("Falha ao conectar ao PostgreSQL")?;

    // Roda migrações embutidas
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .context("Falha ao aplicar migrações")?;

    tracing::info!("PostgreSQL conectado e migrações aplicadas");
    Ok(pool)
}

pub async fn connect_redis(redis_url: &str) -> Result<redis::aio::ConnectionManager> {
    let client = redis::Client::open(redis_url)
        .context("URL do Redis inválida")?;

    let manager = redis::aio::ConnectionManager::new(client)
        .await
        .context("Falha ao conectar ao Redis")?;

    tracing::info!("Redis conectado");
    Ok(manager)
}
