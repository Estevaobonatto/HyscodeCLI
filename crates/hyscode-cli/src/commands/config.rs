use anyhow::Context;
use hyscode_config::{load_config, save_config};

pub async fn run(action: crate::ConfigAction) -> anyhow::Result<()> {
    match action {
        crate::ConfigAction::Get { key } => {
            let config = load_config().unwrap_or_default();
            let value = match key.as_str() {
                "profile.name" => config.profile.name,
                "profile.default_provider" => config.profile.default_provider,
                "profile.default_model" => config.profile.default_model,
                "ui.theme" => config.ui.theme,
                "ui.stream" => config.ui.stream.to_string(),
                "agent.auto_approve" => config.agent.auto_approve.to_string(),
                "agent.max_iterations" => config.agent.max_iterations.to_string(),
                _ => "Chave desconhecida".to_owned(),
            };
            println!("{} = {}", key, value);
        }
        crate::ConfigAction::Set { key, value } => {
            let mut config = load_config().unwrap_or_default();
            let value_str = value.clone();
            match key.as_str() {
                "profile.name" => config.profile.name = value,
                "profile.default_provider" => config.profile.default_provider = value,
                "profile.default_model" => config.profile.default_model = value,
                "ui.theme" => config.ui.theme = value,
                "ui.stream" => config.ui.stream = value.parse().unwrap_or(true),
                "agent.auto_approve" => config.agent.auto_approve = value.parse().unwrap_or(false),
                "agent.max_iterations" => config.agent.max_iterations = value.parse().unwrap_or(15),
                _ => println!("Chave desconhecida: {}", key),
            }
            save_config(&config)?;
            println!("✅ Configuração atualizada: {} = {}", key, value_str);
        }
        crate::ConfigAction::Show => {
            let config = load_config().unwrap_or_default();
            println!("{}", toml::to_string_pretty(&config)?);
        }
        crate::ConfigAction::Edit => {
            let path = hyscode_config::file::config_path();
            println!("Abrindo {} no editor padrão...", path.display());
            #[cfg(unix)]
            {
                let editor = std::env::var("EDITOR")
                    .or_else(|_| std::env::var("VISUAL"))
                    .unwrap_or_else(|_| "vi".to_owned());
                std::process::Command::new(&editor)
                    .arg(&path)
                    .spawn()
                    .context("falha ao abrir editor")?;
            }
            #[cfg(windows)]
            {
                std::process::Command::new("notepad")
                    .arg(&path)
                    .spawn()
                    .context("falha ao abrir editor")?;
            }
        }
    }
    Ok(())
}
