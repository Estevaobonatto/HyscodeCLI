//! Enumerações e constantes do domínio.

use serde::{Deserialize, Serialize};

/// Identificador de provedor suportado.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    #[serde(rename = "copilot")]
    GitHubCopilot,
    OpenRouter,
    #[serde(rename = "zai")]
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

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Aliases de modelo por intenção de uso.
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

/// Modo de operação do agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    /// Modo Planejamento: apenas análise, leitura e criação de planos.
    #[default]
    Plan,
    /// Modo Build: implementação e execução.
    Build,
    /// Modo Review: análise de código, debug, git, PRs e issues.
    Review,
}

impl AgentMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Build => "build",
            Self::Review => "review",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Plan => "PLAN",
            Self::Build => "BUILD",
            Self::Review => "REVIEW",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Plan => "Planejamento: análise e leitura apenas",
            Self::Build => "Build: implementação e execução",
            Self::Review => "Review: análise, debug e revisão",
        }
    }

    pub fn all() -> &'static [AgentMode] {
        &[AgentMode::Plan, AgentMode::Build, AgentMode::Review]
    }

    pub fn next(self) -> Self {
        match self {
            Self::Plan => Self::Build,
            Self::Build => Self::Review,
            Self::Review => Self::Plan,
        }
    }
}

impl std::fmt::Display for AgentMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// SSE — sentinel de fim de stream.
pub const SSE_DONE_SENTINEL: &str = "[DONE]";
pub const SSE_DATA_PREFIX: &str = "data: ";
