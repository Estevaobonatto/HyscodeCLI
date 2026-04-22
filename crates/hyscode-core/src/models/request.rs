//! Tipos de requisição ao provedor.

use crate::models::{message::Message, tool::ToolDefinition};

/// Requisição de chat no formato canônico interno.
#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDefinition>>,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub stop: Option<Vec<String>>,
    pub stream: bool,
    /// ID do usuário final (para logging e monitoramento).
    pub user: Option<String>,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            messages,
            tools: None,
            temperature: None,
            max_tokens: None,
            top_p: None,
            stop: None,
            stream: false,
            user: None,
        }
    }

    pub fn with_stream(mut self) -> Self {
        self.stream = true;
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}
