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

/// Preço por modelo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPricing {
    pub currency: String,
    pub unit: String,
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cached_input: Option<f64>,
    pub long_context_input: Option<f64>,
    pub long_context_cached_input: Option<f64>,
    pub long_context_output: Option<f64>,
    pub audio_input: Option<f64>,
    pub image_output: Option<f64>,
}

/// Informações sobre um modelo disponível num provedor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub context_window: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub capabilities: ProviderCapabilities,
    pub pricing: Option<ModelPricing>,
}
