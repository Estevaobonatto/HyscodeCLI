use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

use super::app::ChatApp;
use super::components::{
    draw_chat_area, draw_command_palette, draw_header, draw_help, draw_input_bar, draw_modal,
};

pub fn draw(frame: &mut Frame, app: &mut ChatApp) {
    let area = frame.size();

    // Layout principal: header (2 linhas) | chat | input (3 linhas) | footer (1 linha)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // header minimalista
            Constraint::Min(1),    // chat area
            Constraint::Length(3), // input bar
            Constraint::Length(1), // footer sutil
        ])
        .split(area);

    draw_header(frame, app, main_chunks[0]);
    draw_chat_area(frame, app, main_chunks[1]);
    draw_input_bar(frame, app, main_chunks[2]);

    if app.command_palette_selection.is_some() {
        draw_command_palette(frame, app, main_chunks[2]);
    }

    draw_footer(frame, app, main_chunks[3]);

    if let Some(ref modal) = app.modal {
        draw_modal(frame, app, modal.clone(), area);
    }

    if app.show_help {
        draw_help(frame, app, area);
    }
}

fn draw_footer(frame: &mut Frame, app: &ChatApp, area: ratatui::layout::Rect) {
    use ratatui::style::{Color, Style};
    use ratatui::widgets::Paragraph;
    let theme = app.theme;
    let status = match app.status {
        super::app::AppStatus::Idle => "pronto",
        super::app::AppStatus::Loading => "carregando...",
        super::app::AppStatus::Streaming => "recebendo...",
        super::app::AppStatus::Error => "erro",
    };
    let text = format!(" {} ", status);
    let footer = Paragraph::new(text).style(
        Style::default()
            .fg(theme.fg_muted())
            .bg(Color::Rgb(15, 15, 20)),
    );
    frame.render_widget(footer, area);
}
