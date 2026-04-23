//! Comando `hyscode review` — revisa o diff atual com LLM.

use std::sync::Arc;

use anyhow::Context;
use futures::stream::StreamExt;
use hyscode_config::load_config;
use hyscode_core::{
    models::{message::Message, request::ChatRequest},
    traits::provider::Provider,
};

use super::providers::{build_registry, resolve_model_alias};

/// Prompt de revisão de código injetado como system message.
const REVIEW_SYSTEM_PROMPT: &str = r#"Você é um revisor de código experiente especializado em Rust e boas práticas de engenharia.
Analise o diff fornecido e produza uma revisão clara e objetiva com:
1. **Resumo** das mudanças (1-2 frases)
2. **Problemas** encontrados (bugs, issues de segurança, performance)
3. **Sugestões** de melhoria
4. **Pontos positivos**
Seja específico, cite linhas ou trechos quando relevante. Use markdown."#;

pub async fn run(
    staged: bool,
    provider_override: Option<String>,
    model_override: Option<String>,
) -> anyhow::Result<()> {
    // Obtém o diff do repositório
    let diff = get_diff(staged).await?;

    if diff.trim().is_empty() {
        if staged {
            println!("Nenhuma mudança staged. Use `git add` primeiro.");
        } else {
            println!("Nenhuma mudança não-commitada encontrada.");
        }
        return Ok(());
    }

    let config = load_config().unwrap_or_default();

    let provider_name = provider_override
        .or_else(|| std::env::var("HYSCODE_PROVIDER").ok())
        .unwrap_or_else(|| config.profile.default_provider.clone());

    let raw_model = model_override
        .or_else(|| std::env::var("HYSCODE_MODEL").ok())
        .unwrap_or_else(|| config.profile.default_model.clone());
    let model = resolve_model_alias(&raw_model, &provider_name);

    let registry = build_registry(&config).await?;
    let provider = registry
        .get(&provider_name)
        .or_else(|| registry.default_provider())
        .context("Nenhum provedor configurado. Use `hyscode provider add`.")?;

    println!("Revisando diff com {} ({})...\n", provider_name, model);

    let diff_preview = &diff[..diff.len().min(32_000)];
    let user_content = format!("Revise este diff:\n\n```diff\n{}\n```", diff_preview);

    let messages = vec![
        Message::System { content: REVIEW_SYSTEM_PROMPT.to_owned() },
        Message::User { content: user_content.into() },
    ];

    let request = ChatRequest::new(&model, messages).with_stream();

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            while let Some(chunk) = stream.next().await {
                if let Ok(c) = chunk {
                    if let Some(text) = c.delta.content {
                        print!("{}", text);
                    }
                }
            }
            println!();
        }
        Err(e) => anyhow::bail!("Erro ao contatar o provedor: {}", e),
    }

    Ok(())
}

async fn get_diff(staged: bool) -> anyhow::Result<String> {
    let args = if staged {
        vec!["diff", "--staged"]
    } else {
        vec!["diff", "HEAD"]
    };

    let out = tokio::process::Command::new("git")
        .args(&args)
        .output()
        .await?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("git diff falhou: {}", err);
    }

    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
