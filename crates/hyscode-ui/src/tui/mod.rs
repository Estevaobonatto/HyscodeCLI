pub mod app;
pub mod components;
pub mod events;
pub mod theme;
pub mod ui;

use std::time::Duration;

use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

use app::ChatApp;
use events::{handle_event, read_event};

/// Executa o loop principal da aplicação TUI.
pub async fn run_app(app: &mut ChatApp) -> anyhow::Result<Option<String>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app_loop(&mut terminal, app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn run_app_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut ChatApp,
) -> anyhow::Result<Option<String>> {
    let mut last_message: Option<String> = None;

    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        let event = read_event(Duration::from_millis(50))?;
        if let Some(ev) = event {
            let has_new_message = handle_event(app, ev)?;
            if has_new_message {
                if let Some(msg) = app.messages.last() {
                    last_message = Some(msg.raw_content.clone());
                }
                return Ok(last_message);
            }
        }

        app.tick();

        if app.exit {
            return Ok(None);
        }
    }
}

/// Atualiza o estado da TUI sem bloquear (para streaming).
pub fn draw_frame<B: Backend>(terminal: &mut Terminal<B>, app: &mut ChatApp) -> anyhow::Result<()> {
    terminal.draw(|f| ui::draw(f, app))?;
    Ok(())
}
