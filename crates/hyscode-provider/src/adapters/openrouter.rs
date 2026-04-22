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
        // Delega para o inner OpenAIAdapter que chama GET /models
        self.inner.list_models().await
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        self.inner.validate().await
    }
}
