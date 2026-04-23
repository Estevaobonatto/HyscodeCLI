//! Adapter para a OpenRouter API.
//!
//! OpenRouter é um gateway que unifica múltiplos provedores de LLM
//! com um único protocolo OpenAI-compatible.
//! Referência: https://openrouter.ai/docs

use async_trait::async_trait;
use futures::stream::BoxStream;
use hyscode_core::{
    error::ProviderError,
    models::{
        provider::{ModelInfo, ProviderCapabilities},
        request::ChatRequest,
        response::{ChatChunk, ChatResponse},
    },
    traits::provider::Provider,
};

use crate::adapters::openai::{OpenAIAdapter, OpenAIConfig};

/// Configuração do adapter OpenRouter.
#[derive(Debug, Clone)]
pub struct OpenRouterConfig {
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for OpenRouterConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            default_model: "openai/gpt-4o".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para a API do OpenRouter.
///
/// Suporta centenas de modelos de múltiplos provedores via protocolo
/// OpenAI-compatible. Veja https://openrouter.ai/models para lista completa.
pub struct OpenRouterAdapter {
    inner: OpenAIAdapter,
}

impl OpenRouterAdapter {
    pub fn new(config: OpenRouterConfig) -> Self {
        let openai_config = OpenAIConfig {
            api_key: config.api_key,
            base_url: "https://openrouter.ai/api/v1".to_owned(),
            default_model: config.default_model,
            timeout_secs: config.timeout_secs,
            max_retries: config.max_retries,
        };

        Self {
            inner: OpenAIAdapter::new(openai_config),
        }
    }
}

#[async_trait]
impl Provider for OpenRouterAdapter {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: true,
            // OpenRouter redireciona para modelos com até 2M tokens de contexto.
            max_context_tokens: 1_000_000,
        }
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        self.inner.chat(request).await
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        self.inner.chat_stream(request).await
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let capabilities = self.capabilities();
        Ok(vec![
            ModelInfo {
                id: "anthropic/claude-opus-4-7".to_owned(),
                name: "Claude Opus 4.7".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(5.0),
                    output: Some(25.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "anthropic/claude-sonnet-4-6".to_owned(),
                name: "Claude Sonnet 4.6".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(64_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(3.0),
                    output: Some(15.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "openai/gpt-5.4".to_owned(),
                name: "GPT-5.4".to_owned(),
                context_window: Some(1_050_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(2.5),
                    output: Some(15.0),
                    cached_input: Some(0.25),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "openai/gpt-5.4-mini".to_owned(),
                name: "GPT-5.4 Mini".to_owned(),
                context_window: Some(400_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.75),
                    output: Some(4.5),
                    cached_input: Some(0.075),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "google/gemini-3.1-pro-preview".to_owned(),
                name: "Gemini 3.1 Pro Preview".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(64_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(2.0),
                    output: Some(12.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "google/gemini-3.1-flash-lite-preview".to_owned(),
                name: "Gemini 3.1 Flash-Lite Preview".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(64_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.25),
                    output: Some(1.5),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "google/gemini-3-flash-preview".to_owned(),
                name: "Gemini 3 Flash Preview".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(64_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.5),
                    output: Some(3.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "meta-llama/llama-4-scout".to_owned(),
                name: "Llama 4 Scout".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities: capabilities.clone(),
                pricing: None,
            },
            ModelInfo {
                id: "deepseek/deepseek-r1".to_owned(),
                name: "DeepSeek R1".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities: capabilities.clone(),
                pricing: None,
            },
            ModelInfo {
                id: "mistralai/mistral-large-latest".to_owned(),
                name: "Mistral Large".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities: capabilities.clone(),
                pricing: None,
            },
            ModelInfo {
                id: "z-ai/glm-5.1".to_owned(),
                name: "GLM-5.1".to_owned(),
                context_window: Some(204_800),
                max_output_tokens: Some(131_072),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(1.4),
                    output: Some(4.4),
                    cached_input: Some(0.26),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-5".to_owned(),
                name: "GLM-5".to_owned(),
                context_window: Some(204_800),
                max_output_tokens: Some(131_072),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(1.0),
                    output: Some(3.2),
                    cached_input: Some(0.2),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-5-turbo".to_owned(),
                name: "GLM-5 Turbo".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(1.2),
                    output: Some(4.0),
                    cached_input: Some(0.24),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-5v-turbo".to_owned(),
                name: "GLM-5V Turbo".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(1.2),
                    output: Some(4.0),
                    cached_input: Some(0.24),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-4.7".to_owned(),
                name: "GLM-4.7".to_owned(),
                context_window: Some(204_800),
                max_output_tokens: Some(131_072),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.6),
                    output: Some(2.2),
                    cached_input: Some(0.11),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-4.5".to_owned(),
                name: "GLM-4.5".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.6),
                    output: Some(2.2),
                    cached_input: Some(0.11),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "z-ai/glm-4.5-air".to_owned(),
                name: "GLM-4.5 Air".to_owned(),
                context_window: None,
                max_output_tokens: None,
                capabilities,
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.2),
                    output: Some(1.1),
                    cached_input: Some(0.03),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
        ])
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        self.inner.validate().await
    }
}
