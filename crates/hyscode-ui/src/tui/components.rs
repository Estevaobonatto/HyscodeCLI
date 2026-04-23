use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Wrap,
    },
    Frame,
};

use super::app::{AppStatus, ChatApp, ChatMessage, MessageBlock, MessageRole, Modal, ProviderConfigAction};
use super::theme::Theme;

// ═══════════════════════════════════════════════════════════════════════════════
// Animação
// ═══════════════════════════════════════════════════════════════════════════════

/// Spinner frames para status loading.
const SPINNER: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Frames do cursor de digitação animado.
const TYPING_CURSOR: &[&str] = &["▌", "▐", "│", " "];

fn spinner_frame(frame: u64) -> &'static str {
    SPINNER[(frame as usize) % SPINNER.len()]
}

fn typing_cursor(frame: u64) -> &'static str {
    TYPING_CURSOR[(frame as usize / 2) % TYPING_CURSOR.len()]
}

// ═══════════════════════════════════════════════════════════════════════════════
// Cores de papel por role
// ═══════════════════════════════════════════════════════════════════════════════

fn role_accent(role: MessageRole, _theme: Theme) -> Color {
    match role {
        MessageRole::User => Color::Rgb(139, 233, 253), // ciano
        MessageRole::Assistant => Color::Rgb(80, 250, 123), // verde neon
        MessageRole::System => Color::Rgb(241, 250, 140), // amarelo
        MessageRole::Tool => Color::Rgb(255, 184, 108), // laranja
    }
}

fn role_label(role: MessageRole) -> &'static str {
    match role {
        MessageRole::User => "Você",
        MessageRole::Assistant => "Agente",
        MessageRole::System => "Sistema",
        MessageRole::Tool => "Ferramenta",
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Header minimalista
// ═══════════════════════════════════════════════════════════════════════════════

pub fn draw_header(frame: &mut Frame, app: &ChatApp, area: Rect) {
    let theme = app.theme;

    // Layout: status + info | modo ativo | atalhos
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),
            Constraint::Length(14),
            Constraint::Length(30),
        ])
        .split(area);

    let status_symbol = match app.status {
        AppStatus::Idle => "●",
        AppStatus::Loading => spinner_frame(app.animation_frame),
        AppStatus::Streaming => "◉",
        AppStatus::Error => "✖",
    };

    let status_color = match app.status {
        AppStatus::Idle => theme.success(),
        AppStatus::Loading => theme.warning(),
        AppStatus::Streaming => theme.accent(),
        AppStatus::Error => theme.error(),
    };

    let mode_color = match app.agent_mode {
        hyscode_core::models::enums::AgentMode::Plan => Color::Rgb(255, 184, 108), // laranja
        hyscode_core::models::enums::AgentMode::Build => Color::Rgb(80, 250, 123), // verde neon
        hyscode_core::models::enums::AgentMode::Review => Color::Rgb(139, 233, 253), // ciano
    };

    let left = Paragraph::new(Line::from(vec![
        Span::styled(status_symbol.to_string(), Style::default().fg(status_color)),
        Span::styled(
            format!(
                "  ·  {}  ·  {}  ·  {}",
                app.current_provider,
                app.current_model,
                app.thinking_level.as_str()
            ),
            Style::default().fg(theme.fg_secondary()),
        ),
    ]));
    frame.render_widget(left, chunks[0]);

    // Modo ativo destacado com cor e badge
    let mode_badge = Paragraph::new(Line::from(vec![
        Span::styled("[", Style::default().fg(theme.fg_muted())),
        Span::styled(
            app.mode_display(),
            Style::default().fg(mode_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled("]", Style::default().fg(theme.fg_muted())),
    ]))
    .alignment(Alignment::Center);
    frame.render_widget(mode_badge, chunks[1]);

    let right = Paragraph::new(Line::from(vec![
        Span::styled("tab ", Style::default().fg(theme.fg_muted())),
        Span::styled("modo  ", Style::default().fg(theme.fg_secondary())),
        Span::styled("ctrl+p ", Style::default().fg(theme.fg_muted())),
        Span::styled("comandos", Style::default().fg(theme.fg_secondary())),
    ]))
    .alignment(Alignment::Right);
    frame.render_widget(right, chunks[2]);

    // Linha divisória sutil
    let line_y = area.y + area.height.saturating_sub(1);
    if line_y >= area.y {
        let line = Paragraph::new("").style(Style::default().bg(theme.border()));
        let line_area = Rect {
            x: area.x,
            y: line_y,
            width: area.width,
            height: 1,
        };
        frame.render_widget(line, line_area);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Chat area com mensagens estilizadas
// ═══════════════════════════════════════════════════════════════════════════════

pub fn draw_chat_area(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let theme = app.theme;

    let inner = area.inner(Margin {
        horizontal: 1,
        vertical: 0,
    });

    let mut all_lines: Vec<Line> = Vec::new();

    for msg in &app.messages {
        all_lines.extend(render_message(msg, theme, app.animation_frame));
    }

    // Espaço extra no final para não colar no input
    all_lines.push(Line::from(""));

    let total_lines = all_lines.len();
    let visible_lines = inner.height as usize;
    // Heurística: com Wrap ativo, cada Line pode ocupar até ~3 linhas visuais
    // dependendo do conteúdo. Multiplicamos por um fator de segurança.
    let estimated_visual_lines = total_lines.saturating_mul(3);
    let max_scroll = estimated_visual_lines.saturating_sub(visible_lines);
    let scroll_y = if app.auto_scroll {
        max_scroll
    } else {
        app.scroll.min(max_scroll)
    };

    let text = Text::from(all_lines);
    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y as u16, 0));

    frame.render_widget(paragraph, inner);

    // Scrollbar fina
    let scrollbar = Scrollbar::default()
        .orientation(ScrollbarOrientation::VerticalRight)
        .begin_symbol(None)
        .end_symbol(None)
        .thumb_symbol("│");

    let mut state = ScrollbarState::new(max_scroll);
    state = state.position(scroll_y);
    frame.render_stateful_widget(scrollbar, inner, &mut state);
}

/// Renderiza uma mensagem completa com blocos especializados.
fn render_message(msg: &ChatMessage, theme: Theme, frame: u64) -> Vec<Line<'_>> {
    let accent = role_accent(msg.role, theme);
    let label = role_label(msg.role);
    let mut lines = Vec::new();

    // Header com linha vertical colorida à esquerda
    let header = Line::from(vec![
        Span::styled("│ ", Style::default().fg(accent)),
        Span::styled(
            label,
            Style::default().fg(accent).add_modifier(Modifier::BOLD),
        ),
    ]);
    lines.push(header);

    // Conteúdo de cada bloco
    // Durante streaming, renderiza como texto puro para evitar
    // artefatos do parser markdown com tags incompletas.
    let is_streaming = msg.is_streaming;
    for block in &msg.blocks {
        match block {
            MessageBlock::Text(text) => {
                if is_streaming {
                    for line in text.lines() {
                        lines.push(Line::from(vec![
                            Span::styled("│ ", Style::default().fg(accent)),
                            Span::styled(line.to_string(), Style::default().fg(theme.fg())),
                        ]));
                    }
                } else {
                    let md_lines = crate::markdown::render_markdown_lines(text, theme.fg(), accent);
                    for md_line in md_lines {
                        let mut spans = vec![Span::styled("│ ", Style::default().fg(accent))];
                        spans.extend(
                            md_line
                                .spans
                                .into_iter()
                                .map(|s| Span::styled(s.content, s.style)),
                        );
                        lines.push(Line::from(spans));
                    }
                }
            }
            MessageBlock::Code { lang, code } => {
                lines.extend(render_code_block(lang, code, theme, accent));
            }
            MessageBlock::Diff { lines: diff_lines } => {
                lines.extend(render_diff_block(diff_lines, theme));
            }
            MessageBlock::ToolCall { name, args } => {
                lines.extend(render_tool_call(name, args, theme));
            }
            MessageBlock::ToolResult {
                name,
                content,
                is_error,
            } => {
                lines.extend(render_tool_result(name, content, *is_error, theme));
            }
            MessageBlock::Thinking(text) => {
                lines.extend(render_thinking(text, theme, frame));
            }
        }
    }

    // Cursor de streaming animado
    if msg.is_streaming {
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(accent)),
            Span::styled(
                typing_cursor(frame).to_string(),
                Style::default().fg(theme.accent()),
            ),
        ]));
    }

    // Espaço entre mensagens
    lines.push(Line::from(""));
    lines
}

// ═══════════════════════════════════════════════════════════════════════════════
// Bloco de código
// ═══════════════════════════════════════════════════════════════════════════════

fn render_code_block<'a>(lang: &str, code: &str, theme: Theme, accent: Color) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Borda superior arredondada simulada
    let top = format!("╭─ {} ─{}─", lang, "─".repeat(20));
    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(accent)),
        Span::styled(top, Style::default().fg(theme.fg_muted())),
    ]));

    for line in code.lines() {
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(accent)),
            Span::styled(
                format!("│ {}", line),
                Style::default().fg(theme.fg_secondary()),
            ),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(accent)),
        Span::styled(
            "╰".to_string() + &"─".repeat(24),
            Style::default().fg(theme.fg_muted()),
        ),
    ]));

    lines
}

// ═══════════════════════════════════════════════════════════════════════════════
// Diff
// ═══════════════════════════════════════════════════════════════════════════════

fn render_diff_block<'a>(diff_lines: &[String], theme: Theme) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.accent())),
        Span::styled(
            "╭─ diff ──────────────────",
            Style::default().fg(theme.fg_muted()),
        ),
    ]));

    for line in diff_lines {
        let (prefix, color) = if line.starts_with('+') {
            ("+", theme.diff_add())
        } else if line.starts_with('-') {
            ("-", theme.diff_remove())
        } else {
            (" ", theme.fg_secondary())
        };

        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(theme.accent())),
            Span::styled(format!("│ {}{}", prefix, line), Style::default().fg(color)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.accent())),
        Span::styled(
            "╰────────────────────────",
            Style::default().fg(theme.fg_muted()),
        ),
    ]));

    lines
}

// ═══════════════════════════════════════════════════════════════════════════════
// Tool Call / Tool Result
// ═══════════════════════════════════════════════════════════════════════════════

fn render_tool_call<'a>(name: &str, args: &str, theme: Theme) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.tool_fg())),
        Span::styled(
            format!("◆ Executando: {}", name),
            Style::default()
                .fg(theme.tool_fg())
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    for line in args.lines() {
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(theme.tool_fg())),
            Span::styled(
                format!("  {}", line),
                Style::default().fg(theme.fg_secondary()),
            ),
        ]));
    }

    lines
}

fn render_tool_result<'a>(
    name: &str,
    content: &str,
    is_error: bool,
    theme: Theme,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let color = if is_error {
        theme.error()
    } else {
        theme.success()
    };
    let symbol = if is_error { "✖" } else { "✔" };

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(color)),
        Span::styled(
            format!("{} Resultado: {}", symbol, name),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
    ]));

    for line in content.lines() {
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(color)),
            Span::styled(
                format!("  {}", line),
                Style::default().fg(theme.fg_secondary()),
            ),
        ]));
    }

    lines
}

// ═══════════════════════════════════════════════════════════════════════════════
// Thinking block com animação de digitação
// ═══════════════════════════════════════════════════════════════════════════════

fn render_thinking<'a>(text: &str, theme: Theme, frame: u64) -> Vec<Line<'a>> {
    let mut lines = Vec::new();
    let dots = ".".repeat(((frame as usize / 4) % 4) + 1);

    lines.push(Line::from(vec![
        Span::styled("│ ", Style::default().fg(theme.fg_muted())),
        Span::styled(
            format!("Pensando{}", dots),
            Style::default()
                .fg(theme.fg_muted())
                .add_modifier(Modifier::ITALIC),
        ),
    ]));

    for line in text.lines() {
        lines.push(Line::from(vec![
            Span::styled("│ ", Style::default().fg(theme.fg_muted())),
            Span::styled(
                format!("  {}", line),
                Style::default()
                    .fg(theme.thinking_fg())
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    lines
}

// ═══════════════════════════════════════════════════════════════════════════════
// Input bar minimalista (sem borda, linha vertical acento)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn draw_input_bar(frame: &mut Frame, app: &ChatApp, area: Rect) {
    let theme = app.theme;

    // Linha divisória sutil acima do input
    let sep_y = area.y;
    let sep_area = Rect {
        x: area.x,
        y: sep_y,
        width: area.width,
        height: 1,
    };
    let sep = Paragraph::new("").style(Style::default().bg(theme.border()));
    frame.render_widget(sep, sep_area);

    let input_area = Rect {
        x: area.x,
        y: sep_y + 1,
        width: area.width,
        height: area.height.saturating_sub(1),
    };

    let accent_color = if app.is_input_command() {
        theme.warning()
    } else {
        theme.accent()
    };

    let placeholder = if app.input.is_empty() {
        "Ask anything... \"Fix a TODO in the codebase\""
    } else {
        &app.input
    };

    let input_style = if app.input.is_empty() {
        Style::default().fg(theme.fg_muted())
    } else {
        Style::default().fg(theme.fg())
    };

    let input_line = Line::from(vec![
        Span::styled("│ ", Style::default().fg(accent_color)),
        Span::styled(placeholder.to_string(), input_style),
    ]);

    let input = Paragraph::new(input_line).wrap(Wrap { trim: false });
    frame.render_widget(input, input_area);

    // Cursor posicionado corretamente (considera largura de exibição UTF-8)
    let cursor_display_width = unicode_width::UnicodeWidthStr::width(&app.input[..app.input_cursor.min(app.input.len())]);
    let cursor_x = input_area.x + 2 + cursor_display_width as u16;
    let cursor_y = input_area.y;
    frame.set_cursor(cursor_x, cursor_y);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Modais estilizados
// ═══════════════════════════════════════════════════════════════════════════════

pub fn draw_modal(frame: &mut Frame, app: &mut ChatApp, modal: Modal, area: Rect) {
    match modal {
        Modal::ProviderSelection => draw_provider_selection(frame, app, area),
        Modal::ModelSelection => draw_model_selection(frame, app, area),
        Modal::ConfigPanel => draw_config_panel(frame, app, area),
        Modal::AgentSelection => draw_agent_selection(frame, app, area),
        Modal::ProviderConfig => draw_provider_config(frame, app, area),
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

    let theme = app.theme;
    let providers = ChatApp::available_providers();
    let items: Vec<ListItem> = providers
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            let style = if i == app.popup_selection {
                Style::default()
                    .bg(theme.accent())
                    .fg(Color::Rgb(10, 10, 14))
            } else {
                Style::default().fg(theme.fg())
            };
            ListItem::new(Line::from(Span::styled(format!(" {} ", p), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()))
                .title(Span::styled(
                    " Selecionar Provedor ",
                    Style::default()
                        .fg(theme.accent())
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, area);
}

fn draw_model_selection(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(80, 70, area);
    frame.render_widget(Clear, area);

    let theme = app.theme;
    let models = &app.available_models;

    // Cabeçalho da tabela
    let header_line = Line::from(vec![
        Span::styled(
            format!(" {:<28}", "Modelo"),
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:>10}", "Contexto"),
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:>10}", "Max Out"),
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:>14}", "Input/1M"),
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" {:>14}", "Output/1M"),
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let mut items: Vec<ListItem> = vec![ListItem::new(header_line)];

    items.extend(models.iter().enumerate().map(|(i, m)| {
        let is_selected = i == app.popup_selection;
        let bg = if is_selected {
            theme.accent()
        } else {
            Color::Reset
        };
        let fg = if is_selected {
            Color::Rgb(10, 10, 14)
        } else {
            theme.fg()
        };
        let fg_secondary = if is_selected {
            Color::Rgb(30, 30, 40)
        } else {
            theme.fg_secondary()
        };

        let ctx = m
            .context_window
            .map(|c| format!("{}", c))
            .unwrap_or_else(|| "?".to_owned());
        let max_out = m
            .max_output_tokens
            .map(|c| format!("{}", c))
            .unwrap_or_else(|| "?".to_owned());

        let (input_price, output_price) = match &m.pricing {
            Some(p) => (
                p.input
                    .map(|v| format!("${:.2}", v))
                    .unwrap_or_else(|| "?".to_owned()),
                p.output
                    .map(|v| format!("${:.2}", v))
                    .unwrap_or_else(|| "?".to_owned()),
            ),
            None => ("N/A".to_owned(), "N/A".to_owned()),
        };

        let line = Line::from(vec![
            Span::styled(
                format!(" {:<28}", m.name.chars().take(28).collect::<String>()),
                Style::default().fg(fg).bg(bg),
            ),
            Span::styled(
                format!(" {:>10}", ctx),
                Style::default().fg(fg_secondary).bg(bg),
            ),
            Span::styled(
                format!(" {:>10}", max_out),
                Style::default().fg(fg_secondary).bg(bg),
            ),
            Span::styled(
                format!(" {:>14}", input_price),
                Style::default().fg(fg_secondary).bg(bg),
            ),
            Span::styled(
                format!(" {:>14}", output_price),
                Style::default().fg(fg_secondary).bg(bg),
            ),
        ]);

        ListItem::new(line)
    }));

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()))
                .title(Span::styled(
                    format!(" Selecionar Modelo ({}) ", app.current_provider),
                    Style::default()
                        .fg(theme.accent())
                        .add_modifier(Modifier::BOLD),
                )),
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
        .style(Style::default().fg(theme.fg_muted()));
    frame.render_widget(info, info_area);
}

fn draw_config_panel(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(60, 50, area);
    frame.render_widget(Clear, area);

    let theme = app.theme;
    let config_text = format!(
        " Provedor atual:    {}\n\
         Modelo atual:      {}\n\
         Modo do agente:    {}\n\
         Nível pensamento:  {}\n\
         Tema:              {}\n\
         \n\
         Use /provider para mudar de provedor.\n\
         Use /models para mudar de modelo.\n\
         Use /agent para mudar o modo (Plan/Build/Review).\n\
         Pressione TAB para ciclar modos rapidamente.\n\
         \n\
         As configurações são salvas automaticamente.",
        app.current_provider,
        app.current_model,
        app.mode_display(),
        app.thinking_level.as_str(),
        if app.theme == Theme::Light {
            "claro"
        } else {
            "escuro"
        },
    );

    let paragraph = Paragraph::new(config_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()))
                .title(Span::styled(
                    " Configurações ",
                    Style::default()
                        .fg(theme.accent())
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

fn draw_agent_selection(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(60, 45, area);
    frame.render_widget(Clear, area);

    let theme = app.theme;
    use hyscode_core::models::enums::AgentMode;
    let modes = AgentMode::all();

    let mode_color = |mode: &AgentMode| -> Color {
        match mode {
            AgentMode::Plan => Color::Rgb(255, 184, 108), // laranja
            AgentMode::Build => Color::Rgb(80, 250, 123), // verde neon
            AgentMode::Review => Color::Rgb(139, 233, 253), // ciano
        }
    };

    let items: Vec<ListItem> = modes
        .iter()
        .enumerate()
        .map(|(i, &mode)| {
            let is_selected = i == app.popup_selection;
            let bg = if is_selected {
                theme.accent()
            } else {
                Color::Reset
            };
            let fg = if is_selected {
                Color::Rgb(10, 10, 14)
            } else {
                theme.fg()
            };
            let desc_fg = if is_selected {
                Color::Rgb(30, 30, 40)
            } else {
                theme.fg_muted()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {:<10}", mode.display_name()),
                    Style::default()
                        .fg(if is_selected { fg } else { mode_color(&mode) })
                        .bg(bg)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", mode.description()),
                    Style::default().fg(desc_fg).bg(bg),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()))
                .title(Span::styled(
                    " Selecionar Modo ",
                    Style::default()
                        .fg(theme.accent())
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, area);

    let info_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - 2,
        width: area.width - 4,
        height: 1,
    };
    let info = Paragraph::new("Enter: selecionar | Esc: fechar | Tab: ciclar rápido")
        .style(Style::default().fg(theme.fg_muted()));
    frame.render_widget(info, info_area);
}

fn draw_provider_config(frame: &mut Frame, app: &mut ChatApp, area: Rect) {
    let area = centered_rect(60, 50, area);
    frame.render_widget(Clear, area);

    let theme = app.theme;
    let actions = app.provider_config_actions();
    let provider = app.current_provider.clone();

    let items: Vec<ListItem> = actions
        .iter()
        .enumerate()
        .map(|(i, action)| {
            let (label, desc) = match action {
                ProviderConfigAction::ChangeApiKey => {
                    ("🔑  Alterar API key", "Atualiza a chave armazenada no vault")
                }
                ProviderConfigAction::LoginOAuth => {
                    ("🔐  Login OAuth", "Autentica via GitHub Device Flow")
                }
                ProviderConfigAction::Logout => {
                    ("🚪  Deslogar", "Remove a credencial do vault local")
                }
                ProviderConfigAction::TestConnection => {
                    ("🧪  Testar conexão", "Valida credenciais com o provedor")
                }
            };
            let is_selected = i == app.popup_selection;
            let bg = if is_selected {
                theme.accent()
            } else {
                Color::Reset
            };
            let fg = if is_selected {
                Color::Rgb(10, 10, 14)
            } else {
                theme.fg()
            };
            let desc_fg = if is_selected {
                Color::Rgb(30, 30, 40)
            } else {
                theme.fg_muted()
            };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {:<22}", label),
                    Style::default().fg(fg).bg(bg),
                ),
                Span::styled(
                    format!("  {}", desc),
                    Style::default().fg(desc_fg).bg(bg),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()))
                .title(Span::styled(
                    format!(" Configurar Provedor: {} ", provider),
                    Style::default()
                        .fg(theme.accent())
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .highlight_symbol("▶ ");

    frame.render_widget(list, area);

    let info_area = Rect {
        x: area.x + 2,
        y: area.y + area.height - 2,
        width: area.width - 4,
        height: 1,
    };
    let info = Paragraph::new("Enter: executar ação | Esc: fechar")
        .style(Style::default().fg(theme.fg_muted()));
    frame.render_widget(info, info_area);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Ajuda
// ═══════════════════════════════════════════════════════════════════════════════

pub fn draw_help(frame: &mut Frame, app: &ChatApp, area: Rect) {
    let area = centered_rect(75, 80, area);
    frame.render_widget(Clear, area);

    let theme = app.theme;
    let commands = vec![
        ("/provider", "Seleciona o provedor de LLM"),
        ("/models", "Seleciona o modelo e nível de pensamento"),
        ("/config", "Abre painel de configurações"),
        ("/config-provider", "Gerencia credenciais do provedor"),
        ("/agent", "Muda o modo do agente (Plan/Build/Review)"),
        ("/clear", "Limpa o histórico de chat"),
        ("/help", "Mostra esta ajuda"),
        ("/exit", "Sai da aplicação"),
    ];

    let mut text = Text::from(vec![
        Line::from(Span::styled(
            "Comandos disponíveis",
            Style::default()
                .fg(theme.accent())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ]);

    for (cmd, desc) in commands {
        text.lines.push(Line::from(vec![
            Span::styled(
                format!("{:12}", cmd),
                Style::default().fg(theme.accent_secondary()),
            ),
            Span::styled(desc, Style::default().fg(theme.fg())),
        ]));
    }

    text.lines.push(Line::from(""));
    text.lines.push(Line::from(Span::styled(
        "Modos do Agente (TAB para ciclar):",
        Style::default()
            .fg(theme.accent())
            .add_modifier(Modifier::BOLD),
    )));

    use hyscode_core::models::enums::AgentMode;
    let modes = vec![
        (
            AgentMode::Plan.display_name(),
            AgentMode::Plan.description(),
            Color::Rgb(255, 184, 108),
        ),
        (
            AgentMode::Build.display_name(),
            AgentMode::Build.description(),
            Color::Rgb(80, 250, 123),
        ),
        (
            AgentMode::Review.display_name(),
            AgentMode::Review.description(),
            Color::Rgb(139, 233, 253),
        ),
    ];
    for (name, desc, color) in modes {
        text.lines.push(Line::from(vec![
            Span::styled(
                format!(" {:<8}", name),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ),
            Span::styled(desc, Style::default().fg(theme.fg_secondary())),
        ]));
    }

    text.lines.push(Line::from(""));
    text.lines.push(Line::from(Span::styled(
        "Atalhos:",
        Style::default()
            .fg(theme.accent())
            .add_modifier(Modifier::BOLD),
    )));
    text.lines.push(Line::from(vec![
        Span::styled("TAB       ", Style::default().fg(theme.accent_secondary())),
        Span::styled(
            "Alterna modo Plan → Build → Review",
            Style::default().fg(theme.fg()),
        ),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("↑/↓       ", Style::default().fg(theme.accent_secondary())),
        Span::styled("Scroll das mensagens", Style::default().fg(theme.fg())),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("PgUp/PgDn ", Style::default().fg(theme.accent_secondary())),
        Span::styled("Scroll rápido", Style::default().fg(theme.fg())),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("Esc       ", Style::default().fg(theme.accent_secondary())),
        Span::styled("Fecha modal / ajuda", Style::default().fg(theme.fg())),
    ]));
    text.lines.push(Line::from(vec![
        Span::styled("Ctrl+C    ", Style::default().fg(theme.accent_secondary())),
        Span::styled("Sai da aplicação", Style::default().fg(theme.fg())),
    ]));

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border()))
                .title(Span::styled(
                    " Ajuda ",
                    Style::default()
                        .fg(theme.accent())
                        .add_modifier(Modifier::BOLD),
                )),
        )
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, area);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Command Palette (tooltip de comandos ao digitar /)
// ═══════════════════════════════════════════════════════════════════════════════

pub fn draw_command_palette(frame: &mut Frame, app: &ChatApp, input_area: Rect) {
    let theme = app.theme;
    let filtered = app.filtered_commands();
    if filtered.is_empty() {
        return;
    }

    let sel = app.command_palette_selection.unwrap_or(0);
    let height = (filtered.len() as u16 + 2).min(12);
    let width = 50u16.min(input_area.width.saturating_sub(4)).max(30);

    let y = input_area.y.saturating_sub(height);
    let x = input_area.x + 2;

    let area = Rect {
        x,
        y,
        width,
        height,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border()))
        .style(Style::default().bg(theme.code_bg()));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<Line> = filtered
        .iter()
        .enumerate()
        .map(|(i, (cmd, desc))| {
            let is_selected = i == sel;
            let cmd_style = if is_selected {
                Style::default()
                    .fg(Color::Rgb(10, 10, 14))
                    .bg(theme.accent())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.fg())
            };
            let desc_style = if is_selected {
                Style::default()
                    .fg(Color::Rgb(30, 30, 40))
                    .bg(theme.accent())
            } else {
                Style::default().fg(theme.fg_muted())
            };
            Line::from(vec![
                Span::styled(format!(" {}", cmd), cmd_style),
                Span::styled(format!("  {}", desc), desc_style),
            ])
        })
        .collect();

    let list = Paragraph::new(Text::from(items));
    frame.render_widget(list, inner);
}
