//! Trait de provedor de LLM — porta da arquitetura hexagonal.

use async_trait::async_trait;
use futures::stream::BoxStream;
use crate::{
    error::ProviderError,
    models::{
        provider::{ModelInfo, ProviderCapabilities},
        request::ChatRequest,
        response::{ChatChunk, ChatResponse},
    },
};

/// Porta de saída para provedores de LLM.
///
/// Todos os adapters (OpenAI, Anthropic, etc.) implementam este trait.
/// O Engine depende apenas desta abstração, nunca de um adapter concreto.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Nome identificador do provedor (ex: "openai", "anthropic").
    fn name(&self) -> &str;

    /// Capacidades suportadas pelo provedor.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Envia uma requisição de chat e aguarda a resposta completa.
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError>;

    /// Envia uma requisição de chat e retorna um stream de chunks.
    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError>;

    /// Lista modelos disponíveis no provedor.
    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError>;

    /// Valida credenciais e conectividade do provedor.
    async fn validate(&self) -> Result<(), ProviderError>;

    /// Estima a contagem de tokens para as mensagens dadas.
    /// Retorna um valor aproximado baseado em tokenização padrão.
    fn estimate_tokens(&self, text: &str) -> u32 {
        // Aproximação: ~4 chars por token (heurística geral)
        (text.len() as u32).saturating_div(4).max(1)
    }
}
