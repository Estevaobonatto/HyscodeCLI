use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::time::Duration;

use super::app::{ChatApp, Modal, SlashCommand, ThinkingLevel};

pub enum AppEvent {
    Tick,
    Key(KeyEvent),
}

pub fn read_event(timeout: Duration) -> anyhow::Result<Option<AppEvent>> {
    if event::poll(timeout)? {
        match event::read()? {
            Event::Key(key) => Ok(Some(AppEvent::Key(key))),
            _ => Ok(Some(AppEvent::Tick)),
        }
    } else {
        Ok(Some(AppEvent::Tick))
    }
}

pub fn handle_event(app: &mut ChatApp, event: AppEvent) -> anyhow::Result<bool> {
    match event {
        AppEvent::Key(key) => handle_key(app, key),
        AppEvent::Tick => Ok(false),
    }
}

fn handle_key(app: &mut ChatApp, key: KeyEvent) -> anyhow::Result<bool> {
    // Ignora eventos de tecla solta (Release) e repetição (Repeat)
    // para evitar duplicação de input.
    if key.kind == KeyEventKind::Release || key.kind == KeyEventKind::Repeat {
        return Ok(false);
    }
    // Ctrl+C sempre sai
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        app.exit = true;
        return Ok(true);
    }

    // Se há um modal aberto
    if app.modal.is_some() {
        return handle_modal_key(app, key);
    }

    // Se ajuda está aberta
    if app.show_help {
        if key.code == KeyCode::Esc {
            app.show_help = false;
        }
        return Ok(false);
    }

    // Command palette: intercepta navegação quando input começa com /
    if app.command_palette_selection.is_some() && app.is_input_command() {
        match key.code {
            KeyCode::Esc => {
                app.command_palette_selection = None;
                return Ok(false);
            }
            KeyCode::Up => {
                app.palette_prev();
                return Ok(false);
            }
            KeyCode::Down => {
                app.palette_next();
                return Ok(false);
            }
            KeyCode::Enter => {
                app.palette_select();
                return Ok(false);
            }
            KeyCode::Char(c) => {
                app.insert_char(c);
                app.update_command_palette();
                return Ok(false);
            }
            KeyCode::Backspace => {
                app.backspace();
                app.update_command_palette();
                return Ok(false);
            }
            KeyCode::Delete => {
                app.delete_char();
                app.update_command_palette();
                return Ok(false);
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Enter => {
            app.command_palette_selection = None;
            if let Some(cmd) = SlashCommand::parse(&app.input) {
                app.clear_input();
                handle_slash_command(app, cmd);
            } else if let Some(_text) = app.submit_input() {
                return Ok(true); // sinaliza que há mensagem nova
            }
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
            app.update_command_palette();
        }
        KeyCode::Backspace => {
            app.backspace();
            app.update_command_palette();
        }
        KeyCode::Delete => {
            app.delete_char();
            app.update_command_palette();
        }
        KeyCode::Left => {
            app.move_cursor_left();
        }
        KeyCode::Right => {
            app.move_cursor_right();
        }
        KeyCode::Home => {
            app.move_cursor_home();
        }
        KeyCode::End => {
            app.move_cursor_end();
        }
        KeyCode::Up => {
            app.scroll_up(1);
        }
        KeyCode::Down => {
            app.scroll_down(1);
        }
        KeyCode::PageUp => {
            app.scroll_up(5);
        }
        KeyCode::PageDown => {
            app.scroll_down(5);
        }
        KeyCode::Esc => {
            app.clear_input();
            app.command_palette_selection = None;
        }
        _ => {}
    }

    Ok(false)
}

fn handle_modal_key(app: &mut ChatApp, key: KeyEvent) -> anyhow::Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.close_modal();
        }
        KeyCode::Up => {
            app.modal_scroll_up();
        }
        KeyCode::Down => {
            let max = match app.modal {
                Some(Modal::ProviderSelection) => ChatApp::available_providers().len(),
                Some(Modal::ModelSelection) => {
                    ChatApp::available_models_for_provider(&app.current_provider).len()
                }
                Some(Modal::AgentSelection) => 4,
                _ => 0,
            };
            app.modal_scroll_down(max);
        }
        KeyCode::Enter => {
            if let Some(ref modal) = app.modal {
                match modal {
                    Modal::ProviderSelection => {
                        let providers = ChatApp::available_providers();
                        if let Some(&provider) = providers.get(app.popup_selection) {
                            app.current_provider = provider.to_owned();
                            app.add_system_message(format!("Provedor alterado para: {}", provider));
                            // Reset model to default for new provider
                            let models = ChatApp::available_models_for_provider(provider);
                            if let Some(&model) = models.first() {
                                app.current_model = model.to_owned();
                            }
                        }
                        app.close_modal();
                    }
                    Modal::ModelSelection => {
                        let models = ChatApp::available_models_for_provider(&app.current_provider);
                        if let Some(&model) = models.get(app.popup_selection) {
                            app.current_model = model.to_owned();
                            app.add_system_message(format!(
                                "Modelo alterado para: {} (pensamento: {})",
                                model,
                                app.thinking_level.as_str()
                            ));
                        }
                        app.close_modal();
                    }
                    Modal::ConfigPanel => {
                        app.close_modal();
                    }
                    Modal::AgentSelection => {
                        let agents = ["default", "code-review", "architecture", "debug"];
                        if let Some(&agent) = agents.get(app.popup_selection) {
                            app.set_agent(agent);
                            app.add_system_message(format!("Agente alterado para: {}", agent));
                        }
                        app.close_modal();
                    }
                }
            }
        }
        KeyCode::Char('t') | KeyCode::Char('T') => {
            if matches!(app.modal, Some(Modal::ModelSelection)) {
                let levels = ThinkingLevel::all();
                let current_idx = levels
                    .iter()
                    .position(|&l| l == app.thinking_level)
                    .unwrap_or(0);
                app.thinking_level = levels[(current_idx + 1) % levels.len()];
            }
        }
        _ => {}
    }

    Ok(false)
}

fn handle_slash_command(app: &mut ChatApp, cmd: SlashCommand) {
    match cmd {
        SlashCommand::Provider => {
            app.open_modal(Modal::ProviderSelection);
        }
        SlashCommand::Models => {
            app.open_modal(Modal::ModelSelection);
        }
        SlashCommand::Config => {
            app.open_modal(Modal::ConfigPanel);
        }
        SlashCommand::Agent => {
            app.open_modal(Modal::AgentSelection);
        }
        SlashCommand::Help => {
            app.show_help = true;
        }
        SlashCommand::Clear => {
            app.messages.clear();
            app.add_system_message("Histórico limpo.");
        }
        SlashCommand::Exit => {
            app.exit = true;
        }
        SlashCommand::Unknown(cmd) => {
            app.add_system_message(format!("Comando desconhecido: {}", cmd));
        }
    }
}
