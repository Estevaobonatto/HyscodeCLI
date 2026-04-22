//! Tipos relacionados a provedores de LLM.

use serde::{Deserialize, Serialize};

/// Capacidades declaradas por um provedor ou modelo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    pub supports_tools: bool,
    pub supports_vision: bool,
    pub supports_streaming: bool,
    pub supports_system_prompt: bool,
    pub supports_parallel_tool_calls: bool,
    pub max_context_tokens: u32,
}

/// Informações sobre um modelo disponível num provedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub capabilities: ProviderCapabilities,
}
