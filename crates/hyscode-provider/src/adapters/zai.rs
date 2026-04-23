//! Adapter para a Z.ai API.
//!
//! Z.ai oferece acesso a modelos de LLM via protocolo OpenAI-compatible.
//! Referência: https://docs.z.ai

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

const ZAI_DEFAULT_BASE_URL: &str = "https://api.z.ai/v1";

/// Configuração do adapter Z.ai.
#[derive(Debug, Clone)]
pub struct ZAiConfig {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for ZAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: ZAI_DEFAULT_BASE_URL.to_owned(),
            default_model: "glm-5.1".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para a API da Z.ai.
pub struct ZAiAdapter {
    inner: OpenAIAdapter,
}

impl ZAiAdapter {
    pub fn new(config: ZAiConfig) -> Self {
        let openai_config = OpenAIConfig {
            api_key: config.api_key,
            base_url: config.base_url,
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
impl Provider for ZAiAdapter {
    fn name(&self) -> &str {
        "zai"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: true,
            max_context_tokens: 200_000,
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
                id: "glm-5.1".to_owned(),
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
                id: "glm-5".to_owned(),
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
                id: "glm-5-turbo".to_owned(),
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
                id: "glm-5v-turbo".to_owned(),
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
                id: "glm-4.7".to_owned(),
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
                id: "glm-4.5".to_owned(),
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
                id: "glm-4.5-air".to_owned(),
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
