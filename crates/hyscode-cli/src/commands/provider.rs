use hyscode_config::{load_config, save_config, keyring::{store_api_key, delete_api_key}};

pub async fn run(action: crate::ProviderAction) -> anyhow::Result<()> {
    match action {
        crate::ProviderAction::Add { name, api_key } => {
            let mut config = load_config().unwrap_or_default();
            let key = if let Some(k) = api_key {
                k
            } else {
                println!("Digite a API key para {}: ", name);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                input.trim().to_owned()
            };

            store_api_key(&name, &key)?;
            config.providers.insert(
                name.clone(),
                hyscode_config::file::ProviderConfig {
                    api_key_source: hyscode_config::file::ApiKeySource::Keyring,
                    env_var: None,
                    base_url: None,
                    default_model: "default".to_owned(),
                    timeout_secs: 120,
                    max_retries: 3,
                },
            );
            save_config(&config)?;
            println!("✅ Provedor '{}' configurado com sucesso.", name);
        }
        crate::ProviderAction::List => {
            let config = load_config().unwrap_or_default();
            println!("Provedores configurados:");
            for (name, cfg) in &config.providers {
                println!("  - {} (modelo: {})", name, cfg.default_model);
            }
        }
        crate::ProviderAction::Remove { name } => {
            let mut config = load_config().unwrap_or_default();
            config.providers.remove(&name);
            let _ = delete_api_key(&name);
            save_config(&config)?;
            println!("🗑️  Provedor '{}' removido.", name);
        }
        crate::ProviderAction::Default { name } => {
            let mut config = load_config().unwrap_or_default();
            config.profile.default_provider = name.clone();
            save_config(&config)?;
            println!("✅ Provedor padrão definido: {}", name);
        }
        crate::ProviderAction::Test { name } => {
            println!("🧪 Testando conectividade com '{}'...", name);
            let config = load_config().unwrap_or_default();
            let registry = crate::commands::providers::build_registry(&config).await?;
            let provider = registry
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!(
                    "Provedor '{}' não configurado. Use `hyscode provider add {}`.",
                    name, name
                ))?;
            match provider.validate().await {
                Ok(()) => println!("✅ Conexão com '{}' bem-sucedida!", name),
                Err(e) => println!("❌ Falha ao conectar com '{}': {}", name, e),
            }
        }
        crate::ProviderAction::Login { name } => {
            run_login(name).await?;
        }
        crate::ProviderAction::Models { name } => {
            let config = load_config().unwrap_or_default();
            let registry = crate::commands::providers::build_registry(&config).await?;
            let provider = registry
                .get(&name)
                .ok_or_else(|| anyhow::anyhow!(
                    "Provedor '{}' não configurado.",
                    name
                ))?;
            let models = provider.list_models().await
                .map_err(|e| anyhow::anyhow!("Erro ao listar modelos: {}", e))?;
            if models.is_empty() {
                println!("Nenhum modelo encontrado para '{}'.", name);
            } else {
                println!("Modelos disponíveis em '{}':", name);
                for m in models {
                    println!("  {:40}  ctx: {} tokens", m.id, m.context_window);
                }
            }
        }
    }
    Ok(())
}

pub async fn run_login(name: String) -> anyhow::Result<()> {
    if name != "copilot" {
        println!("⚠️  Login OAuth só é suportado para 'copilot' no momento.");
        return Ok(());
    }

    let token = crate::oauth::authenticate_copilot().await?;
    store_api_key(&name, &token)?;

    let mut config = load_config().unwrap_or_default();
    config.providers.insert(
        name.clone(),
        hyscode_config::file::ProviderConfig {
            api_key_source: hyscode_config::file::ApiKeySource::Keyring,
            env_var: None,
            base_url: Some("https://api.githubcopilot.com".to_owned()),
            default_model: "gpt-4o-copilot".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        },
    );
    save_config(&config)?;

    println!("✅ Provedor '{}' autenticado via OAuth e salvo.", name);
    Ok(())
}
