use hyscode_core::models::usage::TokenUsage;

/// Tema da interface TUI.
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
    pub fn bg(&self) -> ratatui::style::Color {
        match self {
            Theme::Dark => ratatui::style::Color::Black,
            Theme::Light => ratatui::style::Color::White,
        }
    }

    pub fn fg(&self) -> ratatui::style::Color {
        match self {
            Theme::Dark => ratatui::style::Color::White,
            Theme::Light => ratatui::style::Color::Black,
        }
    }

    pub fn fg_secondary(&self) -> ratatui::style::Color {
        match self {
            Theme::Dark => ratatui::style::Color::Gray,
            Theme::Light => ratatui::style::Color::DarkGray,
        }
    }

    pub fn border(&self) -> ratatui::style::Color {
        match self {
            Theme::Dark => ratatui::style::Color::DarkGray,
            Theme::Light => ratatui::style::Color::Gray,
        }
    }

    pub fn highlight(&self) -> ratatui::style::Color {
        ratatui::style::Color::Cyan
    }
}

/// Estado da aplicação de chat TUI.
pub struct ChatApp {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub input_cursor: usize,
    pub scroll: usize,
    pub status: AppStatus,
    pub current_provider: String,
    pub current_model: String,
    pub thinking_level: ThinkingLevel,
    pub modal: Option<Modal>,
    pub popup_selection: usize,
    pub show_help: bool,
    pub exit: bool,
    pub pending_command: Option<SlashCommand>,
    /// System prompt customizado pelo ContextBuilder.
    pub system_prompt: Option<String>,
    /// Uso de tokens da última resposta.
    pub token_usage: Option<TokenUsage>,
    /// Tema atual da interface.
    pub theme: Theme,
    /// Perfil de agente atual.
    pub current_agent: String,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub is_streaming: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppStatus {
    #[default]
    Idle,
    Loading,
    Streaming,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThinkingLevel {
    #[default]
    Default,
    Low,
    Medium,
    High,
}

impl ThinkingLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            ThinkingLevel::Default => "padrão",
            ThinkingLevel::Low => "baixo",
            ThinkingLevel::Medium => "médio",
            ThinkingLevel::High => "alto",
        }
    }

    pub fn all() -> &'static [ThinkingLevel] {
        &[
            ThinkingLevel::Default,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modal {
    ProviderSelection,
    ModelSelection,
    ConfigPanel,
    AgentSelection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    Provider,
    Models,
    Config,
    Agent,
    Help,
    Clear,
    Exit,
    Unknown(String),
}

impl SlashCommand {
    pub fn parse(input: &str) -> Option<Self> {
        if !input.starts_with('/') {
            return None;
        }
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts.first()?;
        Some(match *cmd {
            "/provider" => SlashCommand::Provider,
            "/models" => SlashCommand::Models,
            "/config" => SlashCommand::Config,
            "/agent" => SlashCommand::Agent,
            "/help" => SlashCommand::Help,
            "/clear" => SlashCommand::Clear,
            "/exit" | "/quit" => SlashCommand::Exit,
            other => SlashCommand::Unknown(other.to_owned()),
        })
    }

    pub fn description(&self) -> &'static str {
        match self {
            SlashCommand::Provider => "Seleciona o provedor de LLM",
            SlashCommand::Models => "Seleciona o modelo e nível de pensamento",
            SlashCommand::Config => "Abre painel de configurações",
            SlashCommand::Agent => "Muda o agente/perfis",
            SlashCommand::Help => "Mostra ajuda de comandos",
            SlashCommand::Clear => "Limpa o histórico de chat",
            SlashCommand::Exit => "Sai da aplicação",
            SlashCommand::Unknown(_) => "Comando desconhecido",
        }
    }
}

impl ChatApp {
    pub fn new(provider: String, model: String, theme: Theme) -> Self {
        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            scroll: 0,
            status: AppStatus::Idle,
            current_provider: provider,
            current_model: model,
            thinking_level: ThinkingLevel::Default,
            modal: None,
            popup_selection: 0,
            show_help: false,
            exit: false,
            pending_command: None,
            system_prompt: None,
            token_usage: None,
            theme,
            current_agent: "default".to_owned(),
        };
        app.add_system_message(
            "Bem-vindo ao Hyscode! Digite /help para ver os comandos disponíveis.",
        );
        app
    }

    /// Define o system prompt customizado para esta sessão.
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    /// Altera o perfil de agente e atualiza o system prompt correspondente.
    pub fn set_agent(&mut self, agent: &str) {
        self.current_agent = agent.to_owned();
        let prompt = match agent {
            "code-review" => {
                "Você é um revisor de código especialista. Analise o código focado em: segurança, performance, legibilidade, manutenibilidade e aderência às melhores práticas do ecossistema. Seja direto e actionável."
            }
            "architecture" => {
                "Você é um arquiteto de software sênior. Ajude a projetar sistemas escaláveis, definir boundaries de serviços, escolher tecnologias e criar diagrams de arquitetura conceituais quando útil."
            }
            "debug" => {
                "Você é um especialista em debugging. Analise logs, stack traces e comportamentos anômalos. Sugira hipóteses de causa raiz e passos concretos para validar cada uma."
            }
            _ => {
                "Você é um agente de codificação especializado em Rust. Ajude com tarefas de desenvolvimento, refatoração, testes e documentação de código."
            }
        };
        self.system_prompt = Some(prompt.to_owned());
    }

    /// Atualiza o uso de tokens com dados do último chunk.
    pub fn update_token_usage(&mut self, usage: TokenUsage) {
        self.token_usage = Some(usage);
    }

    pub fn add_message(&mut self, role: MessageRole, content: impl Into<String>) {
        self.messages.push(ChatMessage {
            role,
            content: content.into(),
            is_streaming: false,
        });
        self.scroll_to_bottom();
    }

    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.add_message(MessageRole::System, content);
    }

    pub fn append_to_last(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut() {
            last.content.push_str(text);
        }
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        if let Some(last) = self.messages.last_mut() {
            last.is_streaming = streaming;
        }
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_add(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.scroll = self.scroll.saturating_sub(amount);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        if self.input.len() >= 4096 {
            return;
        }
        self.input.insert(self.input_cursor, c);
        self.input_cursor += 1;
    }

    pub fn backspace(&mut self) {
        if self.input_cursor > 0 {
            self.input_cursor -= 1;
            self.input.remove(self.input_cursor);
        }
    }

    pub fn delete_char(&mut self) {
        if self.input_cursor < self.input.len() {
            self.input.remove(self.input_cursor);
        }
    }

    pub fn move_cursor_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    pub fn move_cursor_right(&mut self) {
        if self.input_cursor < self.input.len() {
            self.input_cursor += 1;
        }
    }

    pub fn move_cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    pub fn move_cursor_end(&mut self) {
        self.input_cursor = self.input.len();
    }

    pub fn clear_input(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
    }

    pub fn submit_input(&mut self) -> Option<String> {
        let text = self.input.trim();
        if text.is_empty() {
            return None;
        }
        let text = text.to_owned();
        self.add_message(MessageRole::User, text.clone());
        self.clear_input();
        Some(text)
    }

    pub fn open_modal(&mut self, modal: Modal) {
        self.modal = Some(modal);
        self.popup_selection = 0;
    }

    pub fn close_modal(&mut self) {
        self.modal = None;
        self.popup_selection = 0;
    }

    pub fn modal_scroll_down(&mut self, max: usize) {
        if self.popup_selection + 1 < max {
            self.popup_selection += 1;
        }
    }

    pub fn modal_scroll_up(&mut self) {
        self.popup_selection = self.popup_selection.saturating_sub(1);
    }

    pub fn is_input_command(&self) -> bool {
        self.input.starts_with('/')
    }

    pub fn available_providers() -> &'static [&'static str] {
        &[
            "openai",
            "anthropic",
            "copilot",
            "openrouter",
            "zai",
            "hyscode",
        ]
    }

    pub fn available_models_for_provider(provider: &str) -> &'static [&'static str] {
        match provider {
            "openai" => &[
                "gpt-4o",
                "gpt-4o-mini",
                "gpt-4-turbo",
                "o1-preview",
                "o1-mini",
            ],
            "anthropic" => &[
                "claude-3-5-sonnet-20241022",
                "claude-3-5-haiku-20241022",
                "claude-3-opus-20240229",
            ],
            "copilot" => &["gpt-4o-copilot", "claude-3.5-sonnet-copilot"],
            "openrouter" => &[
                "openai/gpt-4o",
                "anthropic/claude-3.5-sonnet",
                "google/gemini-pro-1.5",
            ],
            "zai" => &["zai-large"],
            "hyscode" => &["hyscode-smart", "hyscode-fast"],
            _ => &["default"],
        }
    }
}
