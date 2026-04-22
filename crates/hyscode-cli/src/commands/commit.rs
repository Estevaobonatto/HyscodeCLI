//! Comando `hyscode commit` — gera mensagem de commit com LLM.

use hyscode_config::load_config;
use hyscode_core::models::{message::Message, request::ChatRequest};

use super::providers::build_registry;

pub async fn run(all: bool) -> anyhow::Result<()> {
    // 1. Stage arquivos se --all
    if all {
        let output = tokio::process::Command::new("git")
            .args(["add", "-A"])
            .output()
            .await?;
        if !output.status.success() {
            anyhow::bail!("Falha ao fazer git add -A");
        }
        println!("📦 Todos os arquivos modificados foram staged.");
    }

    // 2. Verifica se há alterações staged
    let diff_output = tokio::process::Command::new("git")
        .args(["diff", "--cached"])
        .output()
        .await?;

    let diff = String::from_utf8_lossy(&diff_output.stdout);
    if diff.trim().is_empty() {
        println!("Nenhuma alteração staged para commit.");
        return Ok(());
    }

    let stat_output = tokio::process::Command::new("git")
        .args(["diff", "--cached", "--stat"])
        .output()
        .await?;
    println!("Alterações staged:");
    println!("{}", String::from_utf8_lossy(&stat_output.stdout));

    // 3. Gera mensagem com LLM
    let config = load_config().unwrap_or_default();
    let provider_name = config.profile.default_provider.clone();
    let model = config.profile.default_model.clone();

    let registry = build_registry(&config).await?;
    let provider = registry
        .get(&provider_name)
        .or_else(|| registry.default_provider())
        .ok_or_else(|| anyhow::anyhow!("Nenhum provedor configurado."))?;

    println!("\n🤖 Gerando mensagem de commit...");

    let prompt = format!(
        "Você é um assistente especialista em commits semânticos (Conventional Commits).\n\
         Analise o diff abaixo e gere uma mensagem de commit no formato:\n\n\
         <tipo>(<escopo opcional>): <descrição curta>\n\n\
         Tipos: feat, fix, docs, style, refactor, test, chore, perf, ci\n\n\
         Regras:\n\
         - Use imperativo (ex: 'adiciona', 'corrige', 'remove')\n\
         - Máx 50 chars na primeira linha\n\
         - Seja específico sobre o que mudou\n\
         - Não use ponto final na primeira linha\n\n\
         Diff:\n\
         ```diff\n{}\n```\n\n\
         Responda APENAS com a mensagem de commit, sem formatação extra.",
        diff
    );

    let request = ChatRequest::new(model, vec![
        Message::System { content: "Você gera mensagens de commit no padrão Conventional Commits.".to_owned() },
        Message::User { content: prompt.into() },
    ]);

    match provider.chat(request).await {
        Ok(response) => {
            let message = response.content.unwrap_or_default().trim().to_owned();
            if message.is_empty() {
                println!("⚠️ Não foi possível gerar mensagem.");
                return Ok(());
            }

            println!("\n💡 Mensagem sugerida:\n");
            println!("  {}\n", message);

            // Pergunta se quer usar
            if dialoguer::Confirm::new()
                .with_prompt("Usar esta mensagem?")
                .default(true)
                .interact()?
            {
                let output = tokio::process::Command::new("git")
                    .args(["commit", "-m", &message])
                    .output()
                    .await?;

                if output.status.success() {
                    println!("✅ Commit realizado com sucesso!");
                } else {
                    eprintln!("❌ Falha no git commit: {}", String::from_utf8_lossy(&output.stderr));
                }
            } else {
                println!("Commit cancelado. Use `git commit -m \"sua mensagem\"` manualmente.");
            }
        }
        Err(e) => {
            eprintln!("❌ Erro ao gerar mensagem: {}", e);
        }
    }

    Ok(())
}