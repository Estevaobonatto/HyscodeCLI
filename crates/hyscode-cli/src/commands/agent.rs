//! Comando `hyscode agent` — execução autônoma via AgentLoop harness.

use std::sync::Arc;

use hyscode_config::load_config;
use hyscode_engine::{
    agent::{AgentConfig, AgentEvent, AgentLoop},
    context::ContextBuilder,
    permission::{PermissionCallback, PermissionConfig, PermissionManager},
};
use hyscode_tools::ToolRegistry;
use serde_json::Value;
use tokio::sync::mpsc;

use super::providers::{build_registry, ensure_provider_configured};

pub async fn run(
    task: String,
    auto_approve: bool,
    audit_only: bool,
    provider: Option<String>,
    model: Option<String>,
) -> anyhow::Result<()> {
    let mut config = load_config().unwrap_or_default();
    let provider_name = provider.unwrap_or_else(|| config.profile.default_provider.clone());
    let model = model.unwrap_or_else(|| config.profile.default_model.clone());

    ensure_provider_configured(&provider_name, &mut config).await?;
    let registry = build_registry(&config).await?;
    let provider = registry
        .get(&provider_name)
        .or_else(|| registry.default_provider())
        .ok_or_else(|| {
            anyhow::anyhow!("Nenhum provedor configurado. Use `hyscode provider add`.")
        })?;

    println!("🤖 Agente autônomo iniciado");
    println!("Tarefa: {}", task);
    println!("Provedor: {} | Modelo: {}", provider_name, model);
    println!(
        "Auto-aprovar: {} | Audit-only: {}",
        auto_approve, audit_only
    );
    println!();

    let tool_registry = Arc::new(ToolRegistry::with_defaults());
    let context =
        ContextBuilder::new(std::env::current_dir()?).with_system_prompt(agent_system_prompt());

    let agent_config = AgentConfig {
        model: model.clone(),
        auto_approve,
        audit_only,
        confirm_writes: !auto_approve,
        confirm_commands: !auto_approve,
        max_iterations: 15,
        temperature: 0.2,
        max_tokens: 4096,
    };

    let perm_config = PermissionConfig {
        audit_only,
        auto_approve_reads: !audit_only,
        auto_approve_all: auto_approve,
        confirm_timeout_secs: 60,
    };

    // Event channel para mostrar progresso no terminal
    let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<AgentEvent>();

    tokio::spawn(async move {
        while let Some(ev) = agent_rx.recv().await {
            match ev {
                AgentEvent::IterationStarted { iteration, max } => {
                    println!("  🔄 Iteração {}/{}", iteration, max);
                }
                AgentEvent::ToolExecuting { name, .. } => {
                    println!("  🔧 Executando: {}", name);
                }
                AgentEvent::ToolExecuted {
                    name,
                    success,
                    preview,
                } => {
                    let icon = if success { "✅" } else { "❌" };
                    println!(
                        "  {} {}: {}",
                        icon,
                        name,
                        preview.lines().next().unwrap_or("")
                    );
                }
                AgentEvent::PermissionRequested { tool, .. } => {
                    if !auto_approve && !audit_only {
                        println!("  ⏳ Aguardando confirmação para: {}", tool);
                    }
                }
                AgentEvent::PermissionDenied { tool, reason } => {
                    println!("  🚫 {} negado: {}", tool, reason);
                }
                AgentEvent::LoopFinished {
                    success,
                    iterations,
                } => {
                    let icon = if success { "✅" } else { "⚠️" };
                    println!("  {} Loop finalizado em {} iterações", icon, iterations);
                }
                AgentEvent::LoopError { error } => {
                    println!("  ❌ Erro: {}", error);
                }
                _ => {}
            }
        }
    });

    let pm = if auto_approve {
        PermissionManager::new(
            perm_config,
            Arc::new(hyscode_engine::permission::ApproveAllCallback),
        )
    } else if audit_only {
        PermissionManager::new(
            perm_config,
            Arc::new(hyscode_engine::permission::DenyAllCallback),
        )
    } else {
        PermissionManager::new(perm_config, Arc::new(CliPermissionCallback))
    };

    let agent = AgentLoop::new(provider, tool_registry, context, agent_config)
        .with_permission_manager(pm)
        .with_event_sender(agent_tx);

    let result = agent.run(&task).await?;

    println!();
    if result.success {
        println!("✅ Tarefa concluída com sucesso!");
        if let Some(response) = result.final_response {
            println!("\n{}", response);
        }
    } else {
        println!("⚠️ Tarefa não concluída.");
        if let Some(response) = result.final_response {
            println!("\n{}", response);
        }
    }

    if !result.tools_called.is_empty() {
        println!(
            "\nFerramentas utilizadas: {}",
            result.tools_called.join(", ")
        );
    }

    Ok(())
}

fn agent_system_prompt() -> String {
    r#"Você é o HyscodeCLI Agent, um agente de codificação autônomo.

Suas capacidades:
- Ler arquivos (read_file)
- Escrever/modificar arquivos (write_file)
- Listar diretórios (list_dir)
- Buscar código (search_code)
- Executar comandos shell (execute_command)
- Ver diff git (git_diff)

Regras:
1. Sempre verifique o estado atual antes de modificar
2. Use search_code para encontrar referências
3. Confirme o que vai fazer antes de write_file ou execute_command
4. Se encontrar erro, leia o arquivo relevante e corrija
5. Ao final, explique o que foi feito
6. Nunca execute comandos destrutivos sem confirmar"#
        .to_owned()
}

/// Callback de permissão interativo para o modo CLI.
///
/// Usa `dialoguer::Confirm` para perguntar ao usuário antes de executar
/// ferramentas destrutivas ou que requerem confirmação.
struct CliPermissionCallback;

#[async_trait::async_trait]
impl PermissionCallback for CliPermissionCallback {
    async fn confirm(&self, tool_name: &str, args: &Value, destructive: bool) -> bool {
        let label = if destructive { "[DESTRUTIVO] " } else { "" };
        let args_preview = serde_json::to_string(args)
            .unwrap_or_default()
            .chars()
            .take(120)
            .collect::<String>();
        let prompt = format!(
            "{}Executar '{}' com args: {}?",
            label, tool_name, args_preview
        );
        dialoguer::Confirm::new()
            .with_prompt(&prompt)
            .default(false)
            .interact()
            .unwrap_or(false)
    }
}
