//! Adapter para a GitHub Copilot API.
//!
//! Protocolo OpenAI-compatible com headers adicionais obrigatórios.
//! Autenticação via OAuth Device Flow (ver `hyscode provider login copilot`).

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

/// Configuração do adapter Copilot.
#[derive(Debug, Clone)]
pub struct CopilotConfig {
    /// Token OAuth obtido via GitHub Device Flow.
    pub token: String,
    /// Versão do editor (ex: "vscode/1.85.0").
    pub editor_version: String,
}

impl Default for CopilotConfig {
    fn default() -> Self {
        Self {
            token: String::new(),
            editor_version: "hyscode-cli/1.0.0".to_owned(),
        }
    }
}

/// Adapter para a GitHub Copilot API.
///
/// Usa protocolo OpenAI-compatible mas requer headers específicos do Copilot.
pub struct CopilotAdapter {
    inner: OpenAIAdapter,
}

impl CopilotAdapter {
    pub fn new(config: CopilotConfig) -> Self {
        let openai_config = OpenAIConfig {
            api_key: config.token,
            base_url: "https://api.githubcopilot.com".to_owned(),
            default_model: "gpt-4o".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        };

        Self {
            inner: OpenAIAdapter::new(openai_config),
        }
    }
}

#[async_trait]
impl Provider for CopilotAdapter {
    fn name(&self) -> &str {
        "copilot"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: false,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: false,
            max_context_tokens: 128_000,
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
                id: "gpt-4o".to_owned(),
                name: "GPT-4o (Copilot)".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: None,
                pricing: None,
                capabilities: capabilities.clone(),
            },
            ModelInfo {
                id: "gpt-4o-mini".to_owned(),
                name: "GPT-4o Mini (Copilot)".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: None,
                pricing: None,
                capabilities: capabilities.clone(),
            },
            ModelInfo {
                id: "claude-3.5-sonnet".to_owned(),
                name: "Claude 3.5 Sonnet (Copilot)".to_owned(),
                context_window: Some(200_000),
                max_output_tokens: None,
                pricing: None,
                capabilities,
            },
        ])
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        self.inner.validate().await
    }
}
