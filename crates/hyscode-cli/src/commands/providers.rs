//! Módulo compartilhado para construção do ProviderRegistry.
//!
//! Elimina duplicação entre chat.rs, agent.rs e commit.rs.
//! Usa os adapters corretos para cada provedor.

use std::sync::Arc;

use hyscode_config::{env::api_key_from_env, file::Config, vault::get_api_key};
use hyscode_provider::{
    adapters::{
        anthropic::{AnthropicAdapter, AnthropicConfig},
        gemini::{GeminiAdapter, GeminiConfig},
        openai::{OpenAIAdapter, OpenAIConfig},
        openrouter::{OpenRouterAdapter, OpenRouterConfig},
        zai::{ZAiAdapter, ZAiConfig},
    },
    registry::ProviderRegistry,
};

/// Resolve a API key para um provedor na seguinte ordem de prioridade:
/// 1. Variável de ambiente (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, etc.)
/// 2. Keyring do sistema operacional
/// 3. `env_var` customizada definida no config TOML
pub fn resolve_api_key(provider: &str, config: &Config) -> Option<String> {
    if let Some(key) = api_key_from_env(provider) {
        return Some(key);
    }
    if let Ok(Some(key)) = get_api_key(provider) {
        return Some(key);
    }
    if let Some(pc) = config.providers.get(provider) {
        if let Some(ref env_var) = pc.env_var {
            if let Ok(key) = std::env::var(env_var) {
                return Some(key);
            }
        }
    }
    None
}

/// Constrói um ProviderRegistry com todos os provedores conhecidos.
///
/// Todos os provedores são registrados independentemente de terem API key,
/// para que `list_models` funcione mesmo sem configuração prévia.
/// A API key é usada vazia como fallback; erros de credenciais só ocorrem
/// no momento de enviar requisições de chat.
pub async fn build_registry(config: &Config) -> anyhow::Result<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();

    // OpenAI
    {
        let api_key = resolve_api_key("openai", config).unwrap_or_default();
        let provider_config = config.providers.get("openai");
        let openai = OpenAIAdapter::new(OpenAIConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "gpt-5.4".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("openai", Arc::new(openai));
    }

    // Anthropic — usa AnthropicAdapter nativo (suporta API Messages correta)
    {
        let api_key = resolve_api_key("anthropic", config).unwrap_or_default();
        let provider_config = config.providers.get("anthropic");
        let anthropic = AnthropicAdapter::new(AnthropicConfig {
            api_key,
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "claude-sonnet-4-6".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("anthropic", Arc::new(anthropic));
    }

    // OpenRouter — usa adapter especializado com list_models correta
    {
        let api_key = resolve_api_key("openrouter", config).unwrap_or_default();
        let provider_config = config.providers.get("openrouter");
        let or = OpenRouterAdapter::new(OpenRouterConfig {
            api_key,
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "openai/gpt-5.4".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("openrouter", Arc::new(or));
    }

    // Hyscode — OpenAI-compatible
    {
        let api_key = resolve_api_key("hyscode", config).unwrap_or_default();
        let provider_config = config.providers.get("hyscode");
        let hyscode = OpenAIAdapter::new(OpenAIConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.hyscode.dev/v1".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "hyscode-smart".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("hyscode", Arc::new(hyscode));
    }

    // Copilot — OpenAI-compatible com token OAuth
    {
        let api_key = resolve_api_key("copilot", config).unwrap_or_default();
        let provider_config = config.providers.get("copilot");
        let copilot = OpenAIAdapter::new(OpenAIConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.githubcopilot.com".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "gpt-4o".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("copilot", Arc::new(copilot));
    }

    // Z.ai — usa adapter especializado com list_models correta
    {
        let api_key = resolve_api_key("zai", config).unwrap_or_default();
        let provider_config = config.providers.get("zai");
        let zai = ZAiAdapter::new(ZAiConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.z.ai/v1".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "glm-5.1".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("zai", Arc::new(zai));
    }

    // Google Gemini — adapter nativo
    {
        let api_key = resolve_api_key("gemini", config).unwrap_or_default();
        let provider_config = config.providers.get("gemini");
        let gemini = GeminiAdapter::new(GeminiConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "gemini-2.5-flash".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("gemini", Arc::new(gemini));
    }

    if !config.profile.default_provider.is_empty() {
        registry.set_default(config.profile.default_provider.clone());
    }

    Ok(registry)
}
