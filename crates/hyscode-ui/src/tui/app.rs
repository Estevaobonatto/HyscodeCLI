use hyscode_core::models::usage::TokenUsage;
use crate::tui::theme::Theme;

/// Bloco de conteúdo dentro de uma mensagem de chat.
/// Permite renderização especializada por tipo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageBlock {
    /// Texto simples ou Markdown.
    Text(String),
    /// Bloco de código com linguagem opcional.
    Code { lang: String, code: String },
    /// Diff de código (linhas com prefixo + ou -).
    Diff { lines: Vec<String> },
    /// Chamada de ferramenta.
    ToolCall { name: String, args: String },
    /// Resultado de ferramenta.
    ToolResult { name: String, content: String, is_error: bool },
    /// Bloco de thinking/raciocínio interno.
    Thinking(String),
}

/// Mensagem renderizada no chat.
pub struct ChatMessage {
    pub role: MessageRole,
    pub blocks: Vec<MessageBlock>,
    pub raw_content: String,
    pub is_streaming: bool,
}

impl ChatMessage {
    pub fn text(role: MessageRole, content: impl Into<String>) -> Self {
        let s = content.into();
        Self {
            role,
            raw_content: s.clone(),
            blocks: vec![MessageBlock::Text(s)],
            is_streaming: false,
        }
    }

    pub fn with_blocks(role: MessageRole, blocks: Vec<MessageBlock>, raw: String) -> Self {
        Self {
            role,
            blocks,
            raw_content: raw,
            is_streaming: false,
        }
    }

    pub fn push_text(&mut self, text: &str) {
        self.raw_content.push_str(text);
        if let Some(MessageBlock::Text(ref mut existing)) = self.blocks.last_mut() {
            existing.push_str(text);
        } else {
            self.blocks.push(MessageBlock::Text(text.to_owned()));
        }
    }
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
    pub system_prompt: Option<String>,
    pub token_usage: Option<TokenUsage>,
    pub theme: Theme,
    pub current_agent: String,
    /// Frame de animação (incrementado a cada tick) para efeitos visuais.
    pub animation_frame: u64,
    /// Índice selecionado na palette de comandos (None = fechada).
    pub command_palette_selection: Option<usize>,
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
            animation_frame: 0,
            command_palette_selection: None,
        };
        app.add_system_message(
            "Bem-vindo ao Hyscode! Digite /help para ver os comandos disponíveis.",
        );
        app
    }

    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);
    }

    pub fn available_commands() -> &'static [(&'static str, &'static str)] {
        &[
            ("/provider", "Seleciona o provedor de LLM"),
            ("/models", "Seleciona o modelo e nível de pensamento"),
            ("/config", "Abre painel de configurações"),
            ("/agent", "Muda o agente/perfil"),
            ("/clear", "Limpa o histórico de chat"),
            ("/help", "Mostra ajuda de comandos"),
            ("/exit", "Sai da aplicação"),
        ]
    }

    pub fn filtered_commands(&self) -> Vec<(&'static str, &'static str)> {
        if !self.is_input_command() {
            return Vec::new();
        }
        let query = self.input[1..].to_lowercase();
        Self::available_commands()
            .iter()
            .filter(|(cmd, _)| cmd.to_lowercase().contains(&query))
            .copied()
            .collect()
    }

    pub fn update_command_palette(&mut self) {
        if !self.is_input_command() {
            self.command_palette_selection = None;
            return;
        }
        let filtered = self.filtered_commands();
        if filtered.is_empty() {
            self.command_palette_selection = None;
        } else {
            let sel = self.command_palette_selection.unwrap_or(0)
                .min(filtered.len().saturating_sub(1));
            self.command_palette_selection = Some(sel);
        }
    }

    pub fn palette_prev(&mut self) {
        if let Some(sel) = self.command_palette_selection {
            self.command_palette_selection = Some(sel.saturating_sub(1));
        }
    }

    pub fn palette_next(&mut self) {
        let filtered = self.filtered_commands();
        if let Some(sel) = self.command_palette_selection {
            let new = (sel + 1).min(filtered.len().saturating_sub(1));
            self.command_palette_selection = Some(new);
        }
    }

    pub fn palette_select(&mut self) {
        let filtered = self.filtered_commands();
        if let Some(sel) = self.command_palette_selection {
            if let Some(&(cmd, _)) = filtered.get(sel) {
                self.input = cmd.to_owned();
                self.input_cursor = self.input.len();
                self.command_palette_selection = None;
            }
        }
    }

    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

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

    pub fn update_token_usage(&mut self, usage: TokenUsage) {
        self.token_usage = Some(usage);
    }

    pub fn add_message(&mut self, role: MessageRole, content: impl Into<String>) {
        let text = content.into();
        self.messages.push(ChatMessage::text(role, text));
        self.scroll_to_bottom();
    }

    pub fn add_system_message(&mut self, content: impl Into<String>) {
        self.add_message(MessageRole::System, content);
    }

    pub fn append_to_last(&mut self, text: &str) {
        if let Some(last) = self.messages.last_mut() {
            last.push_text(text);
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
