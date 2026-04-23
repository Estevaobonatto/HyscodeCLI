//! Módulo compartilhado para construção do ProviderRegistry.
//!
//! Elimina duplicação entre chat.rs, agent.rs e commit.rs.
//! Usa os adapters corretos para cada provedor.

use std::io::IsTerminal;
use std::sync::Arc;

use hyscode_config::{
    env::api_key_from_env,
    file::{ApiKeySource, Config, ProviderConfig},
    save_config,
    vault::{delete_api_key, get_api_key, store_api_key},
};
use hyscode_provider::{
    adapters::{
        anthropic::{AnthropicAdapter, AnthropicConfig},
        gemini::{GeminiAdapter, GeminiConfig},
        openai::{OpenAIAdapter, OpenAIConfig},
        openrouter::{OpenRouterAdapter, OpenRouterConfig},
        zai::{ZAiAdapter, ZAiConfig},
        opencode_go::{OpenCodeGoAdapter, OpenCodeGoConfig},
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

    // OpenCode Go — OpenAI-compatible
    {
        let api_key = resolve_api_key("opencode-go", config).unwrap_or_default();
        let provider_config = config.providers.get("opencode-go");
        let opencode_go = OpenCodeGoAdapter::new(OpenCodeGoConfig {
            api_key,
            default_model: provider_config
                .map(|p| p.default_model.clone())
                .unwrap_or_else(|| "opencode-go/kimi-k2.6".to_owned()),
            timeout_secs: provider_config.map(|p| p.timeout_secs).unwrap_or(120),
            max_retries: provider_config.map(|p| p.max_retries).unwrap_or(3),
        });
        registry.register("opencode-go", Arc::new(opencode_go));
    }

    if !config.profile.default_provider.is_empty() {
        registry.set_default(config.profile.default_provider.clone());
    }

    Ok(registry)
}

fn default_model_for_provider(provider: &str) -> String {
    match provider {
        "openai" => "gpt-5.4".to_owned(),
        "anthropic" => "claude-sonnet-4-6".to_owned(),
        "openrouter" => "openai/gpt-5.4".to_owned(),
        "copilot" => "gpt-4o".to_owned(),
        "zai" => "glm-5.1".to_owned(),
        "hyscode" => "hyscode-smart".to_owned(),
        "opencode-go" => "opencode-go/kimi-k2.6".to_owned(),
        _ => "default".to_owned(),
    }
}

/// Verifica se o provedor selecionado possui credenciais configuradas.
/// Se não tiver e o terminal for interativo, solicita a API key ou OAuth ao usuário,
/// salvando a credencial no vault e atualizando o `config.toml` quando necessário.
pub async fn ensure_provider_configured(
    provider_name: &str,
    config: &mut Config,
) -> anyhow::Result<()> {
    if resolve_api_key(provider_name, config).is_some() {
        return Ok(());
    }

    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "Provedor '{}' não possui API key ou OAuth configurado. \
             Use `hyscode provider add {}` ou defina a variável de ambiente.",
            provider_name,
            provider_name
        );
    }

    println!("⚠️  Provedor '{}' não está configurado.", provider_name);

    let key = if provider_name == "copilot" {
        println!("O Copilot requer autenticação via GitHub OAuth.");
        let do_login = dialoguer::Confirm::new()
            .with_prompt("Autenticar via GitHub agora?")
            .default(true)
            .interact()?;
        if do_login {
            let token = crate::oauth::authenticate_copilot().await?;
            store_api_key(provider_name, &token)?;
            String::new() // já salvo no vault
        } else {
            dialoguer::Password::new()
                .with_prompt("Cole o token do GitHub Copilot")
                .interact()?
        }
    } else {
        dialoguer::Password::new()
            .with_prompt(format!(
                "API key para {} (armazenada criptografada localmente)",
                provider_name
            ))
            .interact()?
    };

    if !key.is_empty() {
        store_api_key(provider_name, &key)?;
    }

    // Garante que existe uma entrada no config.toml para o provedor
    if !config.providers.contains_key(provider_name) {
        config.providers.insert(
            provider_name.to_owned(),
            ProviderConfig {
                api_key_source: ApiKeySource::Keyring,
                env_var: None,
                base_url: None,
                default_model: default_model_for_provider(provider_name),
                timeout_secs: 120,
                max_retries: 3,
            },
        );
        save_config(config)?;
    }

    Ok(())
}

/// Solicita uma nova API key ao usuário e a armazena no vault.
/// Se o provedor não existir no config, cria uma entrada padrão.
pub async fn change_provider_api_key(
    provider_name: &str,
    config: &mut Config,
) -> anyhow::Result<String> {
    let key = if provider_name == "copilot" {
        println!("O Copilot requer autenticação via GitHub OAuth.");
        let do_login = dialoguer::Confirm::new()
            .with_prompt("Autenticar via GitHub agora?")
            .default(true)
            .interact()?;
        if do_login {
            let token = crate::oauth::authenticate_copilot().await?;
            store_api_key(provider_name, &token)?;
            String::new() // já salvo no vault
        } else {
            dialoguer::Password::new()
                .with_prompt("Cole o token do GitHub Copilot")
                .interact()?
        }
    } else {
        dialoguer::Password::new()
            .with_prompt(format!(
                "Nova API key para {} (armazenada criptografada localmente)",
                provider_name
            ))
            .interact()?
    };

    if !key.is_empty() {
        store_api_key(provider_name, &key)?;
    }

    if !config.providers.contains_key(provider_name) {
        config.providers.insert(
            provider_name.to_owned(),
            ProviderConfig {
                api_key_source: ApiKeySource::Keyring,
                env_var: None,
                base_url: None,
                default_model: default_model_for_provider(provider_name),
                timeout_secs: 120,
                max_retries: 3,
            },
        );
        save_config(config)?;
    }

    Ok("✅ API key atualizada com sucesso.".to_owned())
}

/// Remove a credencial do provedor do vault local.
pub fn logout_provider(provider_name: &str) -> String {
    match delete_api_key(provider_name) {
        Ok(()) => format!("🚪 Credenciais de '{}' removidas do vault.", provider_name),
        Err(e) => format!("⚠️  Erro ao remover credenciais: {}", e),
    }
}

/// Testa a conectividade com o provedor usando as credenciais atuais.
pub async fn test_provider_connection(
    provider_name: &str,
    config: &Config,
) -> anyhow::Result<String> {
    let registry = build_registry(config).await?;
    let provider = registry
        .get(provider_name)
        .ok_or_else(|| anyhow::anyhow!("Provedor '{}' não encontrado no registry.", provider_name))?;
    match provider.validate().await {
        Ok(()) => Ok(format!("✅ Conexão com '{}' bem-sucedida!", provider_name)),
        Err(e) => Ok(format!("❌ Falha ao conectar com '{}': {}", provider_name, e)),
    }
}
