//! Configuração do gateway carregada de variáveis de ambiente.

use anyhow::{bail, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub listen_addr: String,
    pub jwt_secret: String,
    /// Mapeamento model_alias → upstream_url|provider
    /// Formato: "claude-3-5-sonnet=anthropic,gpt-4o=openai"
    pub model_routes: Vec<ModelRoute>,
}

#[derive(Debug, Clone)]
pub struct ModelRoute {
    pub model: String,
    pub provider: String,
}

impl Config {
    /// Carrega configuração de variáveis de ambiente.
    pub fn from_env() -> Result<Self> {
        let database_url = required("DATABASE_URL")?;
        let redis_url = required("REDIS_URL")?;
        let listen_addr = std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_owned());
        let jwt_secret = required("JWT_SECRET")?;

        let routes_raw = std::env::var("MODEL_ROUTES").unwrap_or_default();
        let model_routes = parse_model_routes(&routes_raw);

        Ok(Self {
            database_url,
            redis_url,
            listen_addr,
            jwt_secret,
            model_routes,
        })
    }
}

fn required(key: &str) -> Result<String> {
    std::env::var(key).map_err(|_| anyhow::anyhow!("Variável de ambiente obrigatória ausente: {}", key))
}

fn parse_model_routes(raw: &str) -> Vec<ModelRoute> {
    raw.split(',')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let model = parts.next()?.trim().to_owned();
            let provider = parts.next()?.trim().to_owned();
            if model.is_empty() || provider.is_empty() {
                return None;
            }
            Some(ModelRoute { model, provider })
        })
        .collect()
}
