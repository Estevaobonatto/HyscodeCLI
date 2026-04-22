//! Módulo compartilhado para construção do ProviderRegistry.
//!
//! Elimina duplicação entre chat.rs, agent.rs e commit.rs.
//! Usa os adapters corretos para cada provedor.

use std::sync::Arc;

use hyscode_config::{
    env::api_key_from_env,
    file::Config,
    keyring::get_api_key,
};
use hyscode_provider::{
    adapters::{
        anthropic::{AnthropicAdapter, AnthropicConfig},
        openai::{OpenAIAdapter, OpenAIConfig},
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

/// Constrói um ProviderRegistry com todos os provedores configurados.
///
/// Cada provedor é registrado somente se uma API key for encontrada.
/// Usa os adapters nativos de cada provedor (ex: AnthropicAdapter para "anthropic").
pub async fn build_registry(config: &Config) -> anyhow::Result<ProviderRegistry> {
    let mut registry = ProviderRegistry::new();

    // OpenAI
    if let Some(api_key) = resolve_api_key("openai", config) {
        let provider_config = config.providers.get("openai");
        let openai = OpenAIAdapter::new(OpenAIConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.openai.com/v1".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "gpt-4o".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("openai", Arc::new(openai));
    }

    // Anthropic — usa AnthropicAdapter nativo (suporta API Messages correta)
    if let Some(api_key) = resolve_api_key("anthropic", config) {
        let provider_config = config.providers.get("anthropic");
        let anthropic = AnthropicAdapter::new(AnthropicConfig {
            api_key,
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "claude-3-5-sonnet-20241022".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("anthropic", Arc::new(anthropic));
    }

    // OpenRouter — OpenAI-compatible
    if let Some(api_key) = resolve_api_key("openrouter", config) {
        let provider_config = config.providers.get("openrouter");
        let or = OpenAIAdapter::new(OpenAIConfig {
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_owned(),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "openai/gpt-4o".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("openrouter", Arc::new(or));
    }

    // Hyscode — OpenAI-compatible
    if let Some(api_key) = resolve_api_key("hyscode", config) {
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
    if let Some(api_key) = resolve_api_key("copilot", config) {
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

    // Z.ai — OpenAI-compatible
    if let Some(api_key) = resolve_api_key("zai", config) {
        let provider_config = config.providers.get("zai");
        let zai = OpenAIAdapter::new(OpenAIConfig {
            api_key,
            base_url: provider_config
                .and_then(|p| p.base_url.clone())
                .unwrap_or_else(|| "https://api.z.ai/v1".to_owned()),
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "z-pro".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("zai", Arc::new(zai));
    }

    if !config.profile.default_provider.is_empty() {
        registry.set_default(config.profile.default_provider.clone());
    }

    Ok(registry)
}
