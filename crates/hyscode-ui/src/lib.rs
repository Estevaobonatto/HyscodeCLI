//! hyscode-ui — Interface do terminal

pub mod markdown;
pub mod stream;
pub mod tui;

pub use stream::StreamRenderer;
pub use tui::app::ChatApp;
pub use tui::events::AppEvent;
pub use tui::run_app;
pub use tui::theme::Theme;
