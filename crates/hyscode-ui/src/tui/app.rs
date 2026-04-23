use crate::tui::theme::Theme;
use hyscode_core::models::enums::AgentMode;
use hyscode_core::models::provider::ModelInfo;
use hyscode_core::models::usage::TokenUsage;

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
    ToolResult {
        name: String,
        content: String,
        is_error: bool,
    },
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
            SlashCommand::Agent => "Muda o modo do agente (Plan/Build/Review)",
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
    /// Modo de operação atual do agente (Plan / Build / Review).
    pub agent_mode: AgentMode,
    /// Contexto de ambiente (OS, shell, cwd, git) pré-computado para concatenar ao system prompt.
    pub environment_context: Option<String>,
    /// Modelos disponíveis para o provedor atual (buscados dinamicamente do provider).
    pub available_models: Vec<ModelInfo>,
    /// Frame de animação (incrementado a cada tick) para efeitos visuais.
    pub animation_frame: u64,
    /// Índice selecionado na palette de comandos (None = fechada).
    pub command_palette_selection: Option<usize>,
    /// Quando Some, indica que o provedor mudou e os modelos devem ser recarregados.
    pub needs_provider_refresh: Option<String>,
}

/// System prompt base aplicado a TODOS os modos de agente.
/// Contém identidade, capacidades, segurança e boas práticas universais.
const BASE_SYSTEM_PROMPT: &str = r#"# IDENTIDADE
Você é o HyscodeCLI, um agente de codificação especializado executando no terminal do usuário.
Você opera dentro de um workspace Rust e deve respeitar rigorosamente as regras abaixo.

# FERRAMENTAS DISPONÍVEIS
Você tem acesso às seguintes ferramentas (tools):
- read_file: ler conteúdo de arquivos
- write_file: criar ou sobrescrever arquivos
- list_dir: listar diretórios
- search_code: buscar texto/padrões no código
- execute_command: executar comandos shell (com timeout de 30s, máx 300s)
- git_diff: obter diffs do repositório git

# REGRAS DE SEGURANÇA (INQUEBRÁVEIS)
1. NUNCA exponha, logue ou envie API keys, tokens ou secrets em mensagens.
2. NUNCA execute comandos destrutivos (rm -rf, format, drop database) sem confirmação explícita do usuário.
3. NUNCA escreva arquivos fora do diretório de trabalho atual (proteção contra path traversal).
4. SEMPRE valide e sanitize paths antes de operações de filesystem.
5. Comandos shell devem ter timeout; se um comando travar, avise o usuário.
6. NUNCA execute comandos construídos com interpolação direta de input do usuário sem sanitização.

# COMO USAR AS FERRAMENTAS
1. SEMPRE leia um arquivo ANTES de modificá-lo. Nunca assuma o conteúdo.
2. Use search_code para encontrar referências, usos e dependências antes de alterar.
3. Ao escrever arquivos, forneça o conteúdo COMPLETO do arquivo, não apenas o diff.
4. Ao executar comandos, explique O QUÊ o comando faz e POR QUÊ é necessário.
5. Quando múltiplas ferramentas forem independentes, prefira executá-las em paralelo (quando suportado).

# MENÇÕES (@)
O usuário pode mencionar arquivos com @caminho/arquivo.rs. Esses arquivos serão lidos automaticamente e injetados no contexto como blocos de código. Use essas menções para focar sua análise.

# FORMATO DE RESPOSTAS
1. Use Markdown para estruturar respostas.
2. Blocos de código devem especificar a linguagem (```rust, ```python, etc.).
3. Seja direto e actionável. Evite verbosity desnecessária.
4. Ao propor mudanças, explique o raciocínio ANTES de mostrar o código.
5. Quando relevante, cite nomes de arquivos e números de linha.

# BOAS PRÁTICAS GERAIS
1. Nunca invente APIs, funções ou bibliotecas que não existem. Verifique antes.
2. Prefira soluções idiomáticas da linguagem/framework do projeto.
3. Se encontrar um erro, analise a causa raiz antes de aplicar correção paliativa.
4. Mantenha compatibilidade com versões existentes; se houver breaking change, avise explicitamente.
5. Respeite .gitignore e não commite arquivos de build ou secrets.
6. Se não souber algo, admita em vez de alucinar.
7. Considere o contexto já fornecido (ambiente, arquivos extras, git diff) como parte da realidade do projeto.

# IDIOMA
Responda no mesmo idioma da mensagem do usuário (português ou inglês), salvo se explicitamente solicitado outro.
"#;

/// Prompt específico de cada modo (apenas a especialização, sem repetir o base).
fn mode_specific_prompt(mode: AgentMode) -> &'static str {
    match mode {
        AgentMode::Plan => {
            r#"# MODO ATUAL: PLAN (Planejamento)

MISSÃO:
Analisar requisitos, ler arquivos existentes e criar um plano de implementação detalhado e acionável.

RESTRIÇÕES ADICIONAIS:
- Você NÃO pode criar, modificar ou excluir arquivos.
- Você NÃO pode executar comandos shell.
- Você NÃO pode realizar alterações no código-fonte.

OBRIGAÇÕES:
- Leia os arquivos relevantes para entender o contexto antes de propor qualquer plano.
- Produza um plano estruturado contendo:
  1. Visão geral do problema
  2. Análise do estado atual (arquivos lidos e suas funções)
  3. Etapas detalhadas de implementação (passo a passo)
  4. Arquivos que serão afetados
  5. Potenciais riscos e considerações
  6. Critérios de aceitação
- Ao final, pergunte explicitamente se o usuário deseja aprovar o plano para execução no modo BUILD.

O plano ficará salvo na memória da conversa como contexto para o modo BUILD."#
        }
        AgentMode::Build => {
            r#"# MODO ATUAL: BUILD (Implementação)

MISSÃO:
Implementar mudanças no código, executar comandos e realizar tarefas de desenvolvimento ativo.

DIRETRIZES:
- Você tem acesso completo a todas as ferramentas.
- Se houver um plano aprovado na conversa, SIGA-O EXATAMENTE ou explique detalhadamente por que está desviando.
- Sempre verifique o estado atual antes de modificar (read_file antes de write_file).
- Use search_code para encontrar referências antes de alterar.
- Confirme o que vai fazer antes de write_file ou execute_command quando a ação for destrutiva.
- Se encontrar erro, leia o arquivo relevante e corrija de forma cirúrgica.
- Ao final de cada tarefa, explique o que foi feito, quais arquivos foram alterados e por quê.
- Prefira soluções simples, idiomáticas e com o menor impacto possível.
- Execute testes (cargo test, etc.) quando disponíveis para validar mudanças."#
        }
        AgentMode::Review => {
            r#"# MODO ATUAL: REVIEW (Revisão e Análise)

MISSÃO:
Analisar código, identificar problemas, revisar mudanças, investigar bugs e avaliar qualidade.

RESTRIÇÕES ADICIONAIS:
- Você NÃO deve modificar arquivos sem permissão explícita do usuário.
- Você NÃO deve executar comandos destrutivos.

DIRETRIZES:
- Análises devem ser profundas e estruturadas por categoria:
  1. SEGURANÇA: vulnerabilidades, injeções, sanitização inadequada, secrets expostos, validação de input
  2. PERFORMANCE: algoritmos ineficientes, alocações desnecessárias, I/O bloqueante, memory leaks, hot paths
  3. LEGIBILIDADE & MANUTENIBILIDADE: nomes confusos, funções longas, acoplamento excessivo, duplicação de código
  4. CORRETUDE: race conditions, erros silenciados, edge cases não tratados, lógica off-by-one
  5. TESTES: cobertura insuficiente, casos de borda ignorados, mocks inadequados, flaky tests
- Para debug: analise logs, stack traces e comportamentos anômalos. Sugira hipóteses de causa raiz e passos concretos para validar cada uma.
- Para git/PR: analise o diff completo, contexto do branch, histórico de commits e possíveis conflitos futuros.
- Seja direto e actionável: cada problema encontrado DEVE ter uma sugestão de correção ou um snippet de código corrigido.
- Priorize os problemas por severidade (crítico, alto, médio, baixo)."#
        }
    }
}

/// Constrói o system prompt completo unindo o base universal com a especialização do modo.
pub fn build_system_prompt_for_mode(mode: AgentMode) -> String {
    format!(
        "{}\n\n{}",
        BASE_SYSTEM_PROMPT.trim(),
        mode_specific_prompt(mode).trim()
    )
}

impl ChatApp {
    pub fn new(
        provider: String,
        model: String,
        available_models: Vec<ModelInfo>,
        theme: Theme,
    ) -> Self {
        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            scroll: 0,
            status: AppStatus::Idle,
            current_provider: provider,
            current_model: model,
            available_models,
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
            agent_mode: AgentMode::Plan,
            environment_context: None,
            animation_frame: 0,
            command_palette_selection: None,
            needs_provider_refresh: None,
        };
        app.add_system_message(
            "Bem-vindo ao Hyscode! [TAB] alterna modo  |  /help para comandos  |  Modo atual: PLAN",
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
            ("/agent", "Muda o modo do agente (Plan/Build/Review)"),
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
            let sel = self
                .command_palette_selection
                .unwrap_or(0)
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
        self.system_prompt = Some(build_system_prompt_for_mode(self.agent_mode));
    }

    pub fn set_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
        self.system_prompt = Some(build_system_prompt_for_mode(mode));
    }

    pub fn cycle_mode(&mut self) -> AgentMode {
        let next = self.agent_mode.next();
        self.set_mode(next);
        next
    }

    pub fn mode_display(&self) -> &'static str {
        self.agent_mode.display_name()
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

    pub fn set_available_models(&mut self, models: Vec<ModelInfo>) {
        self.available_models = models;
        self.popup_selection = 0;
    }

    pub fn available_providers() -> &'static [&'static str] {
        &[
            "openai",
            "anthropic",
            "copilot",
            "openrouter",
            "zai",
            "hyscode",
            "gemini",
        ]
    }
}
