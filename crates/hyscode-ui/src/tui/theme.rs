use ratatui::style::{Color, Modifier, Style};

/// Tema refinado inspirado em interfaces modernas (OpenCode, Cursor, etc.).
/// Fundo preto absoluto, acentos em rosa/magenta, tipografia clean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl std::str::FromStr for Theme {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "light" => Ok(Theme::Light),
            _ => Ok(Theme::Dark),
        }
    }
}

impl Theme {
    /// Fundo preto absoluto.
    pub fn bg(&self) -> Color {
        match self {
            Theme::Dark => Color::Rgb(10, 10, 14),   // quase preto com leve tom azulado
            Theme::Light => Color::Rgb(250, 250, 252),
        }
    }

    /// Texto principal: cinza claro.
    pub fn fg(&self) -> Color {
        match self {
            Theme::Dark => Color::Rgb(220, 220, 230),
            Theme::Light => Color::Rgb(30, 30, 40),
        }
    }

    /// Texto secundário: cinza médio.
    pub fn fg_secondary(&self) -> Color {
        match self {
            Theme::Dark => Color::Rgb(140, 140, 155),
            Theme::Light => Color::Rgb(100, 100, 115),
        }
    }

    /// Texto terciário/muted.
    pub fn fg_muted(&self) -> Color {
        match self {
            Theme::Dark => Color::Rgb(90, 90, 105),
            Theme::Light => Color::Rgb(150, 150, 165),
        }
    }

    /// Destaque principal: rosa/magenta neon.
    pub fn accent(&self) -> Color {
        Color::Rgb(255, 121, 198) // #FF79C6
    }

    /// Destaque secundário: ciano suave.
    pub fn accent_secondary(&self) -> Color {
        Color::Rgb(139, 233, 253) // #8BE9FD
    }

    /// Bordas sutis.
    pub fn border(&self) -> Color {
        match self {
            Theme::Dark => Color::Rgb(45, 45, 58),
            Theme::Light => Color::Rgb(210, 210, 220),
        }
    }

    /// Cor para erros.
    pub fn error(&self) -> Color {
        Color::Rgb(255, 85, 85) // #FF5555
    }

    /// Cor para sucesso/ok.
    pub fn success(&self) -> Color {
        Color::Rgb(80, 250, 123) // #50FA7B
    }

    /// Cor para warnings.
    pub fn warning(&self) -> Color {
        Color::Rgb(241, 250, 140) // #F1FA8C
    }

    /// Cor para blocos de código.
    pub fn code_bg(&self) -> Color {
        Color::Rgb(20, 20, 28)
    }

    /// Cor para diff: adição.
    pub fn diff_add(&self) -> Color {
        Color::Rgb(80, 250, 123)
    }

    /// Cor para diff: remoção.
    pub fn diff_remove(&self) -> Color {
        Color::Rgb(255, 85, 85)
    }

    /// Cor para thinking blocks.
    pub fn thinking_fg(&self) -> Color {
        Color::Rgb(180, 180, 195)
    }

    /// Cor de fundo para tool call cards.
    pub fn tool_bg(&self) -> Color {
        Color::Rgb(25, 25, 38)
    }

    /// Cor de texto para tool call.
    pub fn tool_fg(&self) -> Color {
        Color::Rgb(255, 184, 108) // #FFB86C
    }

    /// Estilo de título bold com acento.
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.accent())
            .add_modifier(Modifier::BOLD)
    }

    /// Estilo de texto normal.
    pub fn text_style(&self) -> Style {
        Style::default().fg(self.fg())
    }

    /// Estilo de texto secundário.
    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.fg_secondary())
    }
}
