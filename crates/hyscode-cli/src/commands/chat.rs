use std::sync::Arc;

use anyhow::Context;
use futures::stream::StreamExt;
use hyscode_config::load_config;
use hyscode_core::{
    models::{message::Message, request::ChatRequest},
    traits::provider::Provider,
};
use hyscode_engine::conversation::ConversationManager;
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

use super::providers::build_registry;

/// Retorna o caminho do banco de dados de conversas.
fn conversations_db_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hyscode")
        .join("conversations.db")
}

pub async fn run(
    message: Option<String>,
    _context: Vec<String>,
    provider_override: Option<String>,
    model_override: Option<String>,
) -> anyhow::Result<()> {
    let config = load_config().unwrap_or_default();

    let provider_name = provider_override
        .or_else(|| std::env::var("HYSCODE_PROVIDER").ok())
        .unwrap_or_else(|| config.profile.default_provider.clone());

    let model = model_override
        .or_else(|| std::env::var("HYSCODE_MODEL").ok())
        .unwrap_or_else(|| config.profile.default_model.clone());

    let registry = build_registry(&config).await?;

    let provider = registry
        .get(&provider_name)
        .or_else(|| registry.default_provider())
        .context("Nenhum provedor configurado. Use /provider ou hyscode provider add.")?;

    // Inicializa o ConversationManager para persistência.
    let conv_manager = ConversationManager::new(conversations_db_path()).await?;
    let conv_id = conv_manager.create(&provider_name, &model).await?;

    // TUI setup
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = ChatApp::new(provider_name.clone(), model.clone());

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
        )
        .await;
        if let Err(e) = result {
            app.add_message(MessageRole::System, format!("Erro: {}", e));
        }
    }

    let result =
        run_chat_loop(&mut terminal, &mut app, &provider, &model, &conv_manager, &conv_id).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_chat_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut ChatApp,
    provider: &Arc<dyn Provider>,
    model: &str,
    conv_manager: &ConversationManager,
    conv_id: &str,
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
) -> anyhow::Result<()> {
    app.status = AppStatus::Loading;
    terminal.draw(|f| draw(f, app))?;

    // Persiste mensagem do usuário antes de enviar ao LLM.
    let user_msg = Message::User {
        content: text.clone().into(),
    };
    let _ = conv_manager.add_message(conv_id, &user_msg).await;

    let messages = build_messages(app);
    let request = ChatRequest::new(model, messages).with_stream();

    app.add_message(MessageRole::Assistant, "");
    app.status = AppStatus::Streaming;
    app.set_streaming(true);

    let mut assistant_content = String::new();

    match provider.chat_stream(request).await {
        Ok(mut stream) => {
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(content) = chunk.delta.content {
                            assistant_content.push_str(&content);
                            app.append_to_last(&content);
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
        }
        Err(e) => {
            app.set_streaming(false);
            app.append_to_last(&format!("\n[Erro: {}]", e));
            app.status = AppStatus::Error;
        }
    }

    // Persiste resposta do assistente.
    if !assistant_content.is_empty() {
        let asst_msg = Message::Assistant {
            content: Some(assistant_content),
            tool_calls: None,
            thinking: None,
        };
        let _ = conv_manager.add_message(conv_id, &asst_msg).await;
    }

    terminal.draw(|f| draw(f, app))?;
    Ok(())
}

fn build_messages(app: &ChatApp) -> Vec<Message> {
    let mut msgs = vec![Message::System {
        content: "Você é um agente de codificação especializado em Rust.".to_owned(),
    }];

    for msg in &app.messages {
        let m = match msg.role {
            MessageRole::User => Message::User {
                content: msg.content.clone().into(),
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
