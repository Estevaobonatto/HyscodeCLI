//! Cliente HTTP compartilhado entre adapters.
//!
//! Centraliza: connection pool, retry com backoff exponencial,
//! logging de requisições, e configuração de TLS.

use std::time::Duration;
use reqwest::{Client, ClientBuilder};

/// Constrói o cliente HTTP padrão para todos os adapters.
pub fn build_client(timeout_secs: u64) -> Client {
    ClientBuilder::new()
        .timeout(Duration::from_secs(timeout_secs))
        .connect_timeout(Duration::from_secs(10))
        .tcp_keepalive(Duration::from_secs(30))
        .pool_max_idle_per_host(10)
        .use_rustls_tls()
        .https_only(true)
        .build()
        .expect("falha ao construir HTTP client")
}

/// Parâmetros de retry com backoff exponencial.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 500,
            max_delay_ms: 30_000,
            backoff_multiplier: 2.0,
        }
    }
}

impl RetryConfig {
    /// Calcula o delay para a tentativa `attempt` (0-indexed).
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let delay_ms = (self.initial_delay_ms as f64
            * self.backoff_multiplier.powi(attempt as i32))
        .min(self.max_delay_ms as f64) as u64;

        Duration::from_millis(delay_ms)
    }
}
