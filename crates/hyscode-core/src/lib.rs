//! hyscode-core — Domínio central do HyscodeCLI
//!
//! Este crate define os tipos, traits e erros que compõem
//! o modelo de domínio da aplicação. Não depende de nenhum
//! outro crate interno.

pub mod error;
pub mod models;
pub mod traits;

// Re-exports principais
pub use error::ProviderError;
pub use models::{
    message::{ContentPart, Message, MessageContent},
    provider::{ModelInfo, ProviderCapabilities},
    request::ChatRequest,
    response::{ChatChunk, ChatResponse, Delta, FinishReason},
    tool::{ToolCall, ToolDefinition, ToolResult},
    usage::TokenUsage,
};
pub use traits::provider::Provider;
