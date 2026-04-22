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
            default_model: "z-pro".to_owned(),
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
        self.inner.list_models().await
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        self.inner.validate().await
    }
}
