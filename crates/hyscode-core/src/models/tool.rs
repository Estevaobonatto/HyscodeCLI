//! Tipos de ferramenta (tool calling).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Chamada de ferramenta solicitada pelo modelo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    /// JSON string com os argumentos (como retornado pelo modelo).
    pub arguments: String,
}

impl ToolCall {
    /// Tenta fazer parse dos argumentos como JSON.
    pub fn parse_args(&self) -> Result<Value, serde_json::Error> {
        serde_json::from_str(&self.arguments)
    }
}

/// Definição de ferramenta enviada ao modelo (JSON Schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    /// JSON Schema dos parâmetros aceitos.
    pub parameters: Value,
}

/// Resultado da execução de uma ferramenta.
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_call_id: String,
    pub content: String,
    pub is_error: bool,
}

impl ToolResult {
    pub fn success(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: content.into(),
            is_error: false,
        }
    }

    pub fn error(tool_call_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            content: message.into(),
            is_error: true,
        }
    }
}
