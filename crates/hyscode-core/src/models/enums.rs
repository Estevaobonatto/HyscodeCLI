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

/// SSE — sentinel de fim de stream.
pub const SSE_DONE_SENTINEL: &str = "[DONE]";
pub const SSE_DATA_PREFIX: &str = "data: ";
