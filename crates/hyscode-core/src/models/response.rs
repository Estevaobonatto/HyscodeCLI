//! Tipos de resposta do provedor.

use crate::models::{tool::ToolCall, usage::TokenUsage};
use serde::{Deserialize, Serialize};

/// Resposta completa (não-streaming).
#[derive(Debug, Clone)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: FinishReason,
    pub usage: TokenUsage,
}

/// Chunk de streaming (Server-Sent Events).
#[derive(Debug, Clone)]
pub struct ChatChunk {
    pub id: String,
    pub delta: Delta,
    pub finish_reason: Option<FinishReason>,
    /// Presente somente no último chunk.
    pub usage: Option<TokenUsage>,
}

/// Delta incremental de um chunk.
#[derive(Debug, Clone, Default)]
pub struct Delta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_call_delta: Option<ToolCallDelta>,
}

/// Delta incremental de um tool_call (chunks acumulam o JSON dos args).
#[derive(Debug, Clone)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments_chunk: Option<String>,
}

/// Motivo pelo qual o modelo parou de gerar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Error,
}
