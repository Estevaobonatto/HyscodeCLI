use std::fs;

use hyscode_config::{
    file::{ApiKeySource, ProviderConfig},
    keyring::store_api_key,
    load_config, save_config,
};

pub async fn run() -> anyhow::Result<()> {
    let dir = std::env::current_dir()?;
    let hyscode_dir = dir.join(".hyscode");

    println!("🚀 Inicializando HyscodeCLI em {}", dir.display());
    println!();

    if hyscode_dir.exists() {
        let overwrite = dialoguer::Confirm::new()
            .with_prompt("Hyscode já está inicializado neste diretório. Reconfigurar?")
            .default(false)
            .interact()?;
        if !overwrite {
            println!("Operação cancelada.");
            return Ok(());
        }
    } else {
        fs::create_dir(&hyscode_dir)?;
    }

    // Selecionar provedor padrão
    let providers = ["openai", "anthropic", "openrouter", "copilot", "zai", "hyscode"];
    let provider_idx = dialoguer::Select::new()
        .with_prompt("Escolha o provedor de LLM padrão")
        .items(&providers)
        .default(0)
        .interact()?;
    let provider = providers[provider_idx];

    // Tratamento especial para Copilot (OAuth)
    let api_key = if provider == "copilot" {
        println!();
        println!("O Copilot requer autenticação via GitHub OAuth.");
        let do_login = dialoguer::Confirm::new()
            .with_prompt("Autenticar via GitHub agora?")
            .default(true)
            .interact()?;
        if do_login {
            crate::commands::provider::run_login("copilot".to_owned()).await?;
            String::new() // Já salvo pelo run_login
        } else {
            dialoguer::Password::new()
                .with_prompt(format!("Cole o token do GitHub Copilot"))
                .interact()?
        }
    } else {
        dialoguer::Password::new()
            .with_prompt(format!("API key para {} (fica armazenada no keyring do SO)", provider))
            .interact()?
    };

    // Salvar API key no keyring se informada
    if !api_key.is_empty() {
        store_api_key(provider, &api_key)?;
    }

    // Modelo padrão para o provedor
    let default_model = default_model_for(provider);
    let model: String = dialoguer::Input::new()
        .with_prompt("Modelo padrão")
        .default(default_model.to_owned())
        .interact_text()?;

    // Atualizar configuração
    let mut config = load_config().unwrap_or_default();
    config.profile.default_provider = provider.to_owned();
    config.profile.default_model = model.clone();
    config.providers.insert(
        provider.to_owned(),
        ProviderConfig {
            api_key_source: ApiKeySource::Keyring,
            env_var: None,
            base_url: None,
            default_model: model,
            timeout_secs: 120,
            max_retries: 3,
        },
    );
    save_config(&config)?;

    // Criar arquivo de contexto do projeto
    let context_file = hyscode_dir.join("context.md");
    if !context_file.exists() {
        fs::write(
            &context_file,
            "# Contexto do projeto\n\nDescreva aqui o contexto do projeto para o agente.\n",
        )?;
    }

    println!();
    println!("✅ HyscodeCLI inicializado com sucesso!");
    println!("   Provedor: {}", provider);
    println!("   Modelo:   {}", config.profile.default_model);
    println!();
    println!("Próximos passos:");
    println!("  hyscode chat          # Inicia o chat interativo");
    println!("  hyscode agent <task>  # Executa uma tarefa autônoma");
    println!("  hyscode config show   # Exibe configuração atual");

    Ok(())
}

fn default_model_for(provider: &str) -> &'static str {
    match provider {
        "openai" => "gpt-4o",
        "anthropic" => "claude-3-5-sonnet-20241022",
        "openrouter" => "openai/gpt-4o",
        "copilot" => "gpt-4o",
        "zai" => "z-pro",
        "hyscode" => "hyscode-smart",
        _ => "default",
    }
}
