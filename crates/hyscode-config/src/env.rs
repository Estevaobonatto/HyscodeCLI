//! Resolução de configuração a partir de variáveis de ambiente.
//!
//! Variáveis de ambiente têm prioridade sobre o arquivo de configuração.

/// Variáveis de ambiente suportadas.
pub const ENV_HYSCODE_API_KEY: &str = "HYSCODE_API_KEY";
pub const ENV_OPENAI_API_KEY: &str = "OPENAI_API_KEY";
pub const ENV_ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";
pub const ENV_OPENROUTER_API_KEY: &str = "OPENROUTER_API_KEY";
pub const ENV_ZAI_API_KEY: &str = "ZAI_API_KEY";
pub const ENV_DEFAULT_PROVIDER: &str = "HYSCODE_PROVIDER";
pub const ENV_DEFAULT_MODEL: &str = "HYSCODE_MODEL";
pub const ENV_LOG_LEVEL: &str = "HYSCODE_LOG";

/// Retorna o nome da variável de ambiente padrão para um provedor.
pub fn env_var_name_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "hyscode" => Some(ENV_HYSCODE_API_KEY),
        "openai" => Some(ENV_OPENAI_API_KEY),
        "anthropic" => Some(ENV_ANTHROPIC_API_KEY),
        "openrouter" => Some(ENV_OPENROUTER_API_KEY),
        "zai" => Some(ENV_ZAI_API_KEY),
        _ => None,
    }
}

/// Tenta obter a API key de um provedor via variável de ambiente.
pub fn api_key_from_env(provider: &str) -> Option<String> {
    let var = env_var_name_for_provider(provider)?;
    std::env::var(var).ok()
}
