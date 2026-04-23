use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};

use super::app::{AppStatus, ChatApp, ChatMessage, MessageRole, Modal};

pub fn draw(frame: &mut Frame, app: &mut ChatApp) {
    let area = frame.size();

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(3)])
        .split(area);

    draw_header(frame, app, main_chunks[0]);
    draw_chat_area(frame, app, main_chunks[1]);
    draw_input_bar(frame, app, main_chunks[2]);

    if let Some(ref modal) = app.modal {
        draw_modal(frame, app, modal.clone(), area);
    }

    if app.show_help {
        draw_help(frame, area);
    }
}

fn draw_header(frame: &mut Frame, app: &ChatApp, area: Rect) {
    let status_color = match app.status {
        AppStatus::Idle => Color::Green,
        AppStatus::Loading => Color::Yellow,
        AppStatus::Streaming => Color::Cyan,
        AppStatus::Error => Color::Red,
    };

    let status_text = match app.status {
        AppStatus::Idle => "●",
        AppStatus::Loading => "◐",
        AppStatus::Streaming => "◉",
        AppStatus::Error => "✖",
    };

    let header_text = format!(
        " {} Hyscode  |  Provedor: {}  |  Modelo: {}  |  Pensamento: {} ",
        status_text,
        app.current_provider,
        app.current_model,
        app.thinking_level.as_str()
    );

    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::White).bg(Color::Black))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(status_color))
                .title(Span::styled(" HyscodeCLI ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        )
        .alignment(Alignment::Center);

    frame.render_widget(header, area);
}

fn draw_chat_area(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(Span::styled(" Chat ", Style::default().fg(Color::Gray)));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let messages: Vec<Line> = app
        .messages
        .iter()
        .flat_map(|msg| render_message(msg))
        .collect();

    let text = Text::from(messages);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((app.scroll as u16, 0));

    frame.render_widget(paragraph, inner);

    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(Some("↑"))
        .end_symbol(Some("↓"));

    let mut state = ScrollbarState::new(app.messages.len().saturating_sub(inner.height as usize));
    state = state.position(app.scroll);
    frame.render_stateful_widget(scrollbar, inner, &mut state);
}

fn render_message(msg: &ChatMessage) -> Vec<Line> {
    let (prefix, color) = match msg.role {
        MessageRole::User => (" Você ", Color::Blue),
        MessageRole::Assistant => (" Agente ", Color::Green),
        MessageRole::System => (" Sistema ", Color::Yellow),
        MessageRole::Tool => (" Ferramenta ", Color::Magenta),
    };

    let mut lines = Vec::new();

    let header = Line::from(vec![
        Span::styled(prefix, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        Span::styled(" ─", Style::default().fg(Color::DarkGray)),
    ]);
    lines.push(header);

    for line in msg.content.lines() {
        lines.push(Line::from(Span::styled(
            format!("  {}", line),
            Style::default().fg(Color::White),
        )));
    }

    if msg.is_streaming {
        lines.push(Line::from(Span::styled(
            "  ▌",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::SLOW_BLINK),
        )));
    }

    lines.push(Line::from(""));
    lines
}

fn draw_input_bar(frame: &mut Frame, app: &ChatApp, area: Rect) {
    let border_color = if app.is_input_command() {
        Color::Yellow
    } else {
        Color::Gray
    };

    let token_info = match &app.token_usage {
        Some(u) if u.total_tokens > 0 => format!(
            " Tokens: {}↑ {}↓ ",
            u.prompt_tokens, u.completion_tokens
        ),
        _ => " Mensagem ".to_owned(),
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(token_info, Style::default().fg(Color::DarkGray)));

    let input = Paragraph::new(app.input.as_str())
        .style(Style::default().fg(Color::White))
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(input, area);

    let cursor_x = area.x + 1 + app.input_cursor as u16;
    let cursor_y = area.y + 1;
    frame.set_cursor(cursor_x, cursor_y);
}

fn draw_modal(frame: &mut Frame, app: &mut ChatApp, modal: Modal, area: Rect) {
    match modal {
        Modal::ProviderSelection => draw_provider_selection(frame, app, area),
        Modal::ModelSelection => draw_model_selection(frame, app, area),
        Modal::ConfigPanel => draw_config_panel(frame, app, area),
        Modal::AgentSelection => draw_agent_selection(frame, app, area),
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_provider_selection(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(60, 50, area);
    frame.render_widget(Clear, area);

    let providers = ChatApp::available_providers();
    let items: Vec<ListItem> = providers
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let style = if i == app.popup_selection {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!(" {} ", p), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Selecionar Provedor ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, area);
}

fn draw_model_selection(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(70, 60, area);
    frame.render_widget(Clear, area);

    let models = ChatApp::available_models_for_provider(&app.current_provider);
    let items: Vec<ListItem> = models
        .iter()
        .enumerate()
        .map(|(i, &m)| {
            let style = if i == app.popup_selection {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!(" {} ", m), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    format!(" Selecionar Modelo ({})", app.current_provider),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, area);

    let info_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - 2,
        width: area.width - 4,
        height: 1,
    };

    let info = Paragraph::new("Tab: nível de pensamento | Enter: selecionar | Esc: fechar")
        .style(Style::default().fg(Color::Gray));
    frame.render_widget(info, info_area);
}

fn draw_config_panel(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(60, 50, area);
    frame.render_widget(Clear, area);

    let config_text = format!(
        " Provedor atual:    {}\n\
         Modelo atual:      {}\n\
         Nível pensamento:  {}\n\
         \n\
         Use /provider para mudar de provedor.\n\
         Use /models para mudar de modelo.\n\
         Use /agent para mudar o perfil.\n\
         \n\
         As configurações são salvas automaticamente.",
        app.current_provider,
        app.current_model,
        app.thinking_level.as_str(),
    );

    let paragraph = Paragraph::new(config_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Configurações ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_agent_selection(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(60, 40, area);
    frame.render_widget(Clear, area);

    let agents = vec!["default", "code-review", "architecture", "debug"];
    let items: Vec<ListItem> = agents
        .iter()
        .enumerate()
        .map(|(i, &a)| {
            let style = if i == app.popup_selection {
                Style::default().bg(Color::Blue).fg(Color::White)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!(" {} ", a), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Selecionar Agente ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, area);
}

fn draw_confirm_modal(frame: &mut Frame, _app: &ChatApp, message: String, area: Rect) {
    let area = centered_rect(60, 30, area);
    frame.render_widget(Clear, area);

    let paragraph = Paragraph::new(message)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Confirmação ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)))
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false })
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

fn draw_help(frame: &mut Frame, area: Rect) {
    let area = centered_rect(70, 70, area);
    frame.render_widget(Clear, area);

    let commands = vec![
        ("/provider", "Seleciona o provedor de LLM"),
        ("/models", "Seleciona o modelo e nível de pensamento"),
        ("/config", "Abre painel de configurações"),
        ("/agent", "Muda o agente/perfil"),
        ("/clear", "Limpa o histórico de chat"),
        ("/help", "Mostra esta ajuda"),
        ("/exit", "Sai da aplicação"),
    ];

    let mut text = Text::from(vec![
        Line::from(Span::styled(
            "Comandos disponíveis",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ]);

    for (cmd, desc) in commands {
        text.lines.push(Line::from(vec![
            Span::styled(format!("{:12}", cmd), Style::default().fg(Color::Yellow)),
            Span::styled(desc, Style::default().fg(Color::White)),
        ]));
    }

    text.lines.push(Line::from(""));
    text.lines.push(Line::from(Span::styled(
        "Atalhos:",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )));
    text.lines.push(Line::from(vec![
        Span::styled("↑/↓       ", Style::default().fg(Color::Yellow)),
        Span::styled("Scroll das mensagens", Style::default().fg(Color::White)),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("PgUp/PgDn ", Style::default().fg(Color::Yellow)),
        Span::styled("Scroll rápido", Style::default().fg(Color::White)),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("Esc       ", Style::default().fg(Color::Yellow)),
        Span::styled("Fecha modal / ajuda", Style::default().fg(Color::White)),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("Ctrl+C    ", Style::default().fg(Color::Yellow)),
        Span::styled("Sai da aplicação", Style::default().fg(Color::White)),
    ]));

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Ajuda ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)))
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}
