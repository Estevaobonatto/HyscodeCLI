//! Erros do domínio.

use thiserror::Error;

/// Erros normalizados de provedor de LLM.
#[derive(Error, Debug)]
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
#[derive(Error, Debug)]
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
#[derive(Error, Debug)]
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
