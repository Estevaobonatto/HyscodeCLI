# Modelos de Dados — HyscodeCLI

> **Propósito:** Definição completa de tipos, schemas e estruturas de dados da CLI e do Provider Service

---

## 1. Modelos do Domínio Central (`hyscode-core`)

### 1.1. Message — Mensagem de Conversa

```rust
/// Representa uma mensagem no histórico de conversa.
/// Formato canônico interno (agnóstico de provedor).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: MessageContent,
    },
    Assistant {
        content: Option<String>,
        tool_calls: Option<Vec<ToolCall>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thinking: Option<String>, // Para modelos com extended thinking
    },
    Tool {
        tool_call_id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    Image { source: ImageSource },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    pub media_type: String, // "image/png", "image/jpeg", etc
    pub data: ImageData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageData {
    Base64(String),
    Url(String),
}
```

### 1.2. ToolCall — Chamada de Ferramenta

```rust
/// Chamada de ferramenta solicitada pelo modelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,         // Identificador único da chamada
    pub name: String,       // Nome da função
    pub arguments: String,  // JSON string com os argumentos
}

/// Definição de uma ferramenta disponível para o modelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}
```

### 1.3. ChatRequest / ChatResponse

```rust
/// Requisição de chat no formato canônico interno.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub stream: bool,
    pub user: Option<String>,
}

/// Resposta completa (não-streaming).
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
}

/// Chunk de streaming.
#[derive(Debug, Clone)]
pub struct ChatChunk {
    pub id: String,
    pub delta: Delta,
    pub finish_reason: Option<FinishReason>,
    pub usage: Option<TokenUsage>, // Presente apenas no último chunk
}

#[derive(Debug, Clone)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_call_delta: Option<ToolCallDelta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

### 1.4. Provider — Abstração de Provedor

```rust
/// Capacidades declaradas por um provedor.
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub supports_streaming: bool,
    pub supports_system_prompt: bool,
    pub max_context_tokens: u32,
    pub supports_parallel_tool_calls: bool,
}

/// Informações sobre um modelo disponível.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub capabilities: ProviderCapabilities,
}

/// Erros normalizados de provedor.
#[derive(thiserror::Error, Debug)]
pub enum ProviderError {
    #[error("provedor '{0}' não está configurado")]
    NotConfigured(String),

    #[error("credenciais inválidas para o provedor '{0}'")]
    InvalidCredentials(String),

    #[error("modelo '{0}' não encontrado")]
    ModelNotFound(String),

    #[error("rate limit excedido; tente novamente em {retry_after_secs}s")]
    RateLimited { retry_after_secs: u64 },

    #[error("limite de contexto excedido: {tokens} tokens (máx: {max})")]
    ContextLengthExceeded { tokens: u32, max: u32 },

    #[error("timeout na requisição ao provedor")]
    Timeout,

    #[error("erro HTTP {status}: {message}")]
    Http { status: u16, message: String },

    #[error("erro no stream de resposta: {0}")]
    StreamError(String),

    #[error("resposta inválida do provedor: {0}")]
    InvalidResponse(String),

    #[error("provedor indisponível temporariamente")]
    Unavailable,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Erros de ferramenta do agente.
#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    #[error("ferramenta '{0}' não encontrada")]
    NotFound(String),

    #[error("argumentos inválidos para '{tool}': {reason}")]
    InvalidArgs { tool: String, reason: String },

    #[error("permissão negada para operação: {0}")]
    PermissionDenied(String),

    #[error("operação cancelada pelo usuário")]
    Cancelled,

    #[error("timeout na execução da ferramenta '{0}'")]
    Timeout(String),

    #[error("erro de I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Erros de configuração.
#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("arquivo de configuração não encontrado em {0}")]
    NotFound(String),

    #[error("erro de parse na configuração: {0}")]
    ParseError(String),

    #[error("chave de API não configurada para o provedor '{0}'")]
    ApiKeyMissing(String),

    #[error("erro de acesso ao keyring: {0}")]
    KeyringError(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

### 1.5. AuditLog — Registro de Auditoria (`hyscode-engine`)

```rust
/// Uma entrada no log de auditoria (formato JSONL).
/// Arquivo: `~/.local/share/hyscode/audit.jsonl`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,           // ISO 8601
    pub action: String,              // ex: "write_file", "execute_command"
    pub args: serde_json::Value,
    pub result: AuditResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuditResult {
    Success,
    Failure { reason: String },
    Denied  { reason: String },
}
```

### 1.6. TokenEstimator (`hyscode-engine`)

```rust
/// Estima e controla o uso de tokens nas requisições.
pub struct TokenEstimator {
    pub model_context_limit: u32,
    /// Reserva 20% do contexto para a resposta.
    pub reserved_for_completion: u32,
}

impl TokenEstimator {
    /// Heurística: ~4 chars por token.
    pub fn estimate(&self, text: &str) -> u32 { ... }

    /// Tokens disponíveis para o prompt (limite − reserva).
    pub fn available_for_prompt(&self) -> u32 { ... }

    /// Se true, o prompt cabe dentro do limite.
    pub fn fits(&self, prompt_tokens: u32) -> bool { ... }

    /// Trunca mensagens para caber no limite,
    /// sempre preservando a primeira (system) e as mais recentes.
    pub fn truncate_messages<T: Clone>(...) -> Vec<T> { ... }
}
```

---

## 2. Modelos de Configuração (`hyscode-config`)

### 2.1. Config TOML — Arquivo de Configuração

```toml
# ~/.config/hyscode/config.toml

[profile]
name = "default"
default_provider = "hyscode"
default_model = "hyscode-smart"

[ui]
theme = "dark"          # "dark" | "light" | "system"
stream = true           # Exibir resposta em streaming
markdown = true         # Renderizar markdown
syntax_highlight = true
show_token_count = true
show_cost = false       # Exibir estimativa de custo
interactive = true      # Modo TUI interativo (vs stream simples)

[agent]
auto_approve = false    # Aprovação automática de ferramentas
audit_only = false      # Apenas mostra ações, não executa
max_iterations = 15     # Máximo de iterações do loop do agente
confirm_writes = true   # Confirmar antes de escrever arquivos
confirm_commands = true # Confirmar antes de executar comandos

[context]
include_git_diff = false      # Incluir diff do stage
max_file_size_kb = 512        # Limite de tamanho de arquivo para contexto
respect_gitignore = true
custom_ignore = [".hyscode/", "*.lock", "target/"]

[providers.openai]
api_key_source = "keyring"    # "keyring" | "env" | "inline" (não recomendado)
env_var = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"  # Pode ser customizado (Azure, etc)
default_model = "gpt-4o"
timeout_secs = 120
max_retries = 3

[providers.anthropic]
api_key_source = "keyring"
env_var = "ANTHROPIC_API_KEY"
default_model = "claude-3-5-sonnet-20241022"
timeout_secs = 120
max_retries = 3

[providers.copilot]
api_key_source = "keyring"
token_type = "oauth"          # "oauth" | "pat"
default_model = "gpt-4o"
timeout_secs = 60
max_retries = 2

[providers.openrouter]
api_key_source = "keyring"
env_var = "OPENROUTER_API_KEY"
default_model = "anthropic/claude-3.5-sonnet"
timeout_secs = 120
max_retries = 3

[providers.zai]
api_key_source = "keyring"
env_var = "ZAI_API_KEY"
base_url = "https://api.z.ai/v1"
default_model = "z1"
timeout_secs = 90
max_retries = 3

[providers.hyscode]
api_key_source = "keyring"
env_var = "HYSCODE_API_KEY"
base_url = "https://api.hyscode.dev/v1"
default_model = "hyscode-smart"
timeout_secs = 120
max_retries = 3
```

### 2.2. Struct de Config em Rust

```rust
/// Configuração raiz lida do TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub profile: ProfileConfig,
    pub ui: UiConfig,
    pub agent: AgentConfig,
    pub context: ContextConfig,
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,
    pub default_provider: String,
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub stream: bool,
    pub markdown: bool,
    pub syntax_highlight: bool,
    pub show_token_count: bool,
    pub show_cost: bool,
    pub interactive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub auto_approve: bool,
    pub audit_only: bool,
    pub max_iterations: u32,
    pub confirm_writes: bool,
    pub confirm_commands: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub include_git_diff: bool,
    pub max_file_size_kb: u64,
    pub respect_gitignore: bool,
    pub custom_ignore: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key_source: ApiKeySource,
    pub env_var: Option<String>,
    pub base_url: Option<String>,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeySource {
    Keyring,
    Env,
    Inline(String), // Não recomendado; aceito por compatibilidade
}
```

---

## 3. Modelos do Banco de Dados (Provider Service)

> **Nota:** O schema abaixo reflete a migration `001_initial.sql` (implementação atual, Fase 4).
> Fases futuras (billing, planos, admin) adicionarão tabelas conforme necessário.

### 3.1. Schema PostgreSQL (migration 001)

```sql
-- Usuários
CREATE TABLE users (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    email         TEXT        NOT NULL UNIQUE,
    password_hash TEXT        NOT NULL,
    display_name  TEXT,
    tier          TEXT        NOT NULL DEFAULT 'free',   -- free | pro | enterprise
    is_active     BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- API Keys
CREATE TABLE api_keys (
    id            UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_hash      TEXT        NOT NULL UNIQUE,        -- SHA-256(key)
    key_prefix    TEXT        NOT NULL,               -- "hsk_..." primeiros 8 chars
    label         TEXT,
    scopes        TEXT[]      NOT NULL DEFAULT '{}',  -- ["chat", "models"]
    rate_limit_rpm INTEGER    NOT NULL DEFAULT 60,
    is_active     BOOLEAN     NOT NULL DEFAULT TRUE,
    last_used_at  TIMESTAMPTZ,
    expires_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_api_keys_key_hash ON api_keys(key_hash);
CREATE INDEX idx_api_keys_user_id  ON api_keys(user_id);

-- Log de requisições
CREATE TABLE requests_log (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    api_key_id        UUID        REFERENCES api_keys(id),
    user_id           UUID        REFERENCES users(id),
    model             TEXT        NOT NULL,
    upstream_provider TEXT        NOT NULL,
    prompt_tokens     INTEGER     NOT NULL DEFAULT 0,
    completion_tokens INTEGER     NOT NULL DEFAULT 0,
    total_tokens      INTEGER     NOT NULL DEFAULT 0,
    latency_ms        INTEGER,
    status_code       SMALLINT    NOT NULL,
    error_message     TEXT,
    requested_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_requests_log_user_id     ON requests_log(user_id);
CREATE INDEX idx_requests_log_requested_at ON requests_log(requested_at DESC);

-- Quotas de uso mensal por usuário
CREATE TABLE usage_quotas (
    id             UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id        UUID        NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    monthly_tokens BIGINT      NOT NULL DEFAULT 0,
    monthly_limit  BIGINT      NOT NULL DEFAULT 1000000,
    reset_at       TIMESTAMPTZ NOT NULL DEFAULT (date_trunc('month', NOW()) + INTERVAL '1 month'),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### 3.2. Estruturas Redis

```
# Rate limiting (Token Bucket por usuário)
Key: rate_limit:{user_id}:rps
Type: Hash
TTL: 60s
Fields:
  tokens: <float>       # Tokens restantes no bucket
  last_refill: <float>  # Timestamp do último refill

# Cache de respostas (opcional, por opt-in do usuário)
Key: response_cache:{sha256(model+messages_normalized)}
Type: String (JSON)
TTL: 3600s (1 hora)
Value: <JSON da ChatResponse>

# Sessões de API Key (cache de validação)
Key: apikey_cache:{key_hash_prefix}
Type: String (JSON)
TTL: 300s (5 min)
Value: {user_id, tier, scopes, is_active}

# Health check de provedores
Key: provider_health:{provider_id}
Type: String
TTL: 60s
Value: "healthy" | "degraded" | "down"
```

---

## 4. Modelos de Histórico Local (SQLite)

```sql
-- Threads de conversa
CREATE TABLE conversations (
    id           TEXT PRIMARY KEY,   -- ulid
    title        TEXT,               -- Gerado automaticamente ou definido pelo usuário
    provider     TEXT NOT NULL,
    model        TEXT NOT NULL,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at   INTEGER NOT NULL DEFAULT (unixepoch())
);

-- Mensagens
CREATE TABLE messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,   -- 'system', 'user', 'assistant', 'tool'
    content         TEXT NOT NULL,
    tool_calls      TEXT,            -- JSON serializado
    tool_call_id    TEXT,
    is_error        INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE INDEX idx_messages_conversation_id ON messages(conversation_id);
```

---

## 5. Modelos de Ferramentas (`hyscode-tools`)

```rust
/// Resultado da execução de uma ferramenta.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

/// Definição de uma ferramenta interna da CLI.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> serde_json::Value;       // JSON Schema dos parâmetros

    fn requires_confirmation(&self) -> bool { false }
    fn is_destructive(&self) -> bool { false }

    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;
}

/// Registro de ferramentas disponíveis.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

// Ferramentas nativas
pub struct ReadFileTool;
pub struct WriteFileTool;
pub struct ListDirTool;
pub struct SearchCodeTool;
pub struct ExecuteCommandTool;
pub struct GitDiffTool;
pub struct GlobSearchTool;
```

---

## 6. Tipos de Enumeração e Constantes

```rust
// hyscode-core/src/models/enums.rs

/// Provedores suportados.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    GitHubCopilot,
    OpenRouter,
    ZAi,
    Hyscode,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OpenAI => "openai",
            Self::Anthropic => "anthropic",
            Self::GitHubCopilot => "copilot",
            Self::OpenRouter => "openrouter",
            Self::ZAi => "zai",
            Self::Hyscode => "hyscode",
        }
    }
}

/// Aliases de modelo.
pub const MODEL_ALIAS_FAST: &str = "fast";
pub const MODEL_ALIAS_SMART: &str = "smart";
pub const MODEL_ALIAS_ULTRA: &str = "ultra";
pub const MODEL_ALIAS_CODE: &str = "code";

/// Limites padrão.
pub const DEFAULT_MAX_TOKENS: u32 = 8192;
pub const DEFAULT_TEMPERATURE: f32 = 1.0;
pub const DEFAULT_MAX_AGENT_ITERATIONS: u32 = 15;
pub const DEFAULT_TIMEOUT_SECS: u64 = 120;
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Configurações de SSE.
pub const SSE_DONE_SENTINEL: &str = "[DONE]";
pub const SSE_DATA_PREFIX: &str = "data: ";
```
