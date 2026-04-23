use std::sync::Arc;

use anyhow::Context;
use futures::stream::StreamExt;
use hyscode_config::load_config;
use hyscode_core::{
    models::{message::Message, request::ChatRequest, usage::TokenUsage},
    traits::provider::Provider,
};
use hyscode_engine::{
    context::ContextBuilder,
    conversation::ConversationManager,
    maybe_summarize,
    token::TokenEstimator,
};
use hyscode_ui::tui::app::{AppStatus, ChatApp, MessageRole};
use hyscode_ui::tui::events::{handle_event, read_event};
use hyscode_ui::tui::ui::draw;
use ratatui::{
    backend::{Backend, CrosstermBackend},
    crossterm::{
        event::{DisableMouseCapture, EnableMouseCapture},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::time::Duration;

use super::providers::{build_registry, resolve_model_alias};

/// Retorna o caminho do banco de dados de conversas.
fn conversations_db_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hyscode")
        .join("conversations.db")
}

/// Verdadeiro quando rodando em terminal interativo.
fn is_interactive() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

pub async fn run(
    message: Option<String>,
    context: Vec<String>,
    provider_override: Option<String>,
    model_override: Option<String>,
) -> anyhow::Result<()> {
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

    // Inicializa o ConversationManager para persistência.
    let conv_manager = ConversationManager::new(conversations_db_path()).await?;
    let conv_id = conv_manager.create(&provider_name, &model).await?;

    // Monta o ContextBuilder com arquivos extras da flag --context
    let extra_files: Vec<std::path::PathBuf> = context
        .iter()
        .map(|s| std::path::PathBuf::from(s))
        .collect();
    let ctx_builder = Arc::new(
        ContextBuilder::new(std::env::current_dir()?)
            .with_extra_files(extra_files),
    );

    // Modo não-interativo: apenas imprime a resposta sem TUI
    if !is_interactive() {
        let msg = message.unwrap_or_default();
        if msg.is_empty() {
            anyhow::bail!("Modo não-interativo requer uma mensagem como argumento.");
        }
        return run_non_interactive(&provider, &model, msg, &ctx_builder, &conv_manager, &conv_id)
            .await;
    }

    // TUI setup
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Constrói system prompt com contexto de arquivos + ambiente
    let system_prompt = ctx_builder.build_system_prompt().await.unwrap_or_default();
    let mut app = ChatApp::new(provider_name.clone(), model.clone());
    if !system_prompt.is_empty() {
        app.set_system_prompt(system_prompt);
    }

    // Se há mensagem inicial, processa
    if let Some(msg) = message {
        app.add_message(MessageRole::User, msg.clone());
        let result = process_message(
            &provider,
            &model,
            msg,
            &mut terminal,
            &mut app,
            &conv_manager,
            &conv_id,
            &ctx_builder,
        )
        .await;
        if let Err(e) = result {
            app.add_message(MessageRole::System, format!("Erro: {}", e));
        }
    }

    let result = run_chat_loop(
        &mut terminal,
        &mut app,
        &provider,
        &model,
        &conv_manager,
        &conv_id,
        &ctx_builder,
    )
    .await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

/// Modo não-interativo: envia mensagem, imprime resposta em stream para stdout.
async fn run_non_interactive(
    provider: &Arc<dyn Provider>,
    model: &str,
    text: String,
    ctx_builder: &ContextBuilder,
    conv_manager: &ConversationManager,
    conv_id: &str,
) -> anyhow::Result<()> {
    let (clean_text, file_ctx) = ctx_builder.resolve_at_mentions(&text).await;

    let mut messages = Vec::new();
    let system_prompt = ctx_builder.build_system_prompt().await.unwrap_or_default();
    if !system_prompt.is_empty() {
        messages.push(Message::System { content: system_prompt });
    }
    if let Some(ctx) = file_ctx {
        messages.push(Message::System { content: ctx });
    }
    messages.push(Message::User { content: clean_text.clone().into() });

    let _ = conv_manager
        .add_message(conv_id, &Message::User { content: clean_text.into() })
        .await;

    let request = ChatRequest::new(model, messages).with_stream();
    let mut assistant_content = String::new();

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            while let Some(chunk) = stream.next().await {
                if let Ok(c) = chunk {
                    if let Some(text) = c.delta.content {
                        print!("{}", text);
                        assistant_content.push_str(&text);
                    }
                }
            }
            println!();
        }
        Err(e) => anyhow::bail!("Erro ao contatar o provedor: {}", e),
    }

    if !assistant_content.is_empty() {
        let _ = conv_manager
            .add_message(
                conv_id,
                &Message::Assistant {
                    content: Some(assistant_content),
                    tool_calls: None,
                    thinking: None,
                },
            )
            .await;
    }

    Ok(())
}

async fn run_chat_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut ChatApp,
    provider: &Arc<dyn Provider>,
    model: &str,
    conv_manager: &ConversationManager,
    conv_id: &str,
    ctx_builder: &ContextBuilder,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        let event = read_event(Duration::from_millis(50))?;
        if let Some(ev) = event {
            let has_new_message = handle_event(app, ev)?;
            if has_new_message {
                if let Some(last) = app.messages.last() {
                    let text = last.content.clone();
                    if let Err(e) = process_message(
                        provider,
                        model,
                        text,
                        terminal,
                        app,
                        conv_manager,
                        conv_id,
                        ctx_builder,
                    )
                    .await
                    {
                        app.add_message(MessageRole::System, format!("Erro: {}", e));
                    }
                }
            }
        }

        if app.exit {
            return Ok(());
        }
    }
}

async fn process_message<B: Backend>(
    provider: &Arc<dyn Provider>,
    model: &str,
    text: String,
    terminal: &mut Terminal<B>,
    app: &mut ChatApp,
    conv_manager: &ConversationManager,
    conv_id: &str,
    ctx_builder: &ContextBuilder,
) -> anyhow::Result<()> {
    app.status = AppStatus::Loading;
    terminal.draw(|f| draw(f, app))?;

    // Resolve menções @arquivo no texto do usuário
    let (clean_text, file_ctx) = ctx_builder.resolve_at_mentions(&text).await;

    // Atualiza a última mensagem da UI com o texto limpo
    if clean_text != text {
        if let Some(last) = app.messages.last_mut() {
            last.content = clean_text.clone();
        }
    }

    // Persiste mensagem do usuário
    let _ = conv_manager
        .add_message(
            conv_id,
            &Message::User { content: clean_text.clone().into() },
        )
        .await;

    let mut messages = build_messages(app);
    // Injeta contexto de arquivo inline antes da mensagem do usuário
    if let Some(ctx) = file_ctx {
        let insert_pos = messages.len().saturating_sub(1);
        messages.insert(insert_pos, Message::System { content: ctx });
    }

    // Auto-sumarização quando contexto excede 85% do limite
    let estimator = TokenEstimator::new(128_000);
    if let Ok(Some(condensed)) = maybe_summarize(&messages, provider.clone(), model, &estimator).await {
        messages = condensed;
    }

    let request = ChatRequest::new(model, messages).with_stream();

    app.add_message(MessageRole::Assistant, "");
    app.status = AppStatus::Streaming;
    app.set_streaming(true);

    let mut assistant_content = String::new();
    let mut final_usage: Option<TokenUsage> = None;

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(content) = chunk.delta.content {
                            assistant_content.push_str(&content);
                            app.append_to_last(&content);
                        }
                        if let Some(usage) = chunk.usage {
                            final_usage = Some(usage);
                        }
                        if chunk.finish_reason.is_some() {
                            app.set_streaming(false);
                        }
                        terminal.draw(|f| draw(f, app))?;
                    }
                    Err(e) => {
                        app.set_streaming(false);
                        app.append_to_last(&format!("\n[Erro no stream: {}]", e));
                        app.status = AppStatus::Error;
                        terminal.draw(|f| draw(f, app))?;
                        return Ok(());
                    }
                }
            }
            app.set_streaming(false);
            app.status = AppStatus::Idle;
            if let Some(usage) = final_usage {
                app.update_token_usage(usage);
            }
        }
        Err(e) => {
            app.set_streaming(false);
            app.append_to_last(&format!("\n[Erro: {}]", e));
            app.status = AppStatus::Error;
        }
    }

    // Persiste resposta do assistente.
    if !assistant_content.is_empty() {
        let _ = conv_manager
            .add_message(
                conv_id,
                &Message::Assistant {
                    content: Some(assistant_content),
                    tool_calls: None,
                    thinking: None,
                },
            )
            .await;
    }

    terminal.draw(|f| draw(f, app))?;
    Ok(())
}

fn build_messages(app: &ChatApp) -> Vec<Message> {
    use hyscode_core::models::message::MessageContent;

    let mut msgs = Vec::new();

    // System prompt definido pelo ContextBuilder (via app.system_prompt)
    if let Some(ref sp) = app.system_prompt {
        msgs.push(Message::System { content: sp.clone() });
    } else {
        msgs.push(Message::System {
            content: "Você é um agente de codificação especializado em Rust.".to_owned(),
        });
    }

    for msg in &app.messages {
        let m = match msg.role {
            MessageRole::User => Message::User {
                content: MessageContent::Text(msg.content.clone()),
            },
            MessageRole::Assistant => Message::Assistant {
                content: Some(msg.content.clone()),
                tool_calls: None,
                thinking: None,
            },
            _ => continue,
        };
        msgs.push(m);
    }

    msgs
}
