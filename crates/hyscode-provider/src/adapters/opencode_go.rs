//! Adapter para a OpenCode Go API.
//!
//! OpenCode Go é um plano de assinatura da OpenCode que fornece acesso
//! a modelos open source de codificação.
//! Referência: https://opencode.ai/docs/go
//! Endpoint base: https://opencode.ai/zen/go/v1
//!
//! A maioria dos modelos usa `/v1/chat/completions` (OpenAI-compatible).
//! MiniMax M2.7 e M2.5 usam `/v1/messages` (Anthropic-compatible).

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{BoxStream, StreamExt};
use hyscode_core::{
    error::ProviderError,
    models::{
        message::{Message, MessageContent},
        provider::{ModelInfo, ModelPricing, ProviderCapabilities},
        request::ChatRequest,
        response::{ChatChunk, ChatResponse, Delta, FinishReason},
        usage::TokenUsage,
    },
    traits::provider::Provider,
};
use serde::{Deserialize, Serialize};

use crate::adapters::openai::{OpenAIAdapter, OpenAIConfig};

const OPENCODE_GO_BASE_URL: &str = "https://opencode.ai/zen/go/v1";

/// Configuração do adapter OpenCode Go.
#[derive(Debug, Clone)]
pub struct OpenCodeGoConfig {
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for OpenCodeGoConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            default_model: "opencode-go/kimi-k2.6".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para a API do OpenCode Go.
///
/// Suporta modelos open source de codificação curados pela equipe OpenCode.
/// Veja https://opencode.ai/docs/go para lista completa e detalhes de preços.
pub struct OpenCodeGoAdapter {
    inner_openai: OpenAIAdapter,
    inner_messages: OpenCodeGoMessagesAdapter,
}

fn strip_provider_prefix(model: &str) -> String {
    model.strip_prefix("opencode-go/").unwrap_or(model).to_owned()
}

impl OpenCodeGoAdapter {
    pub fn new(config: OpenCodeGoConfig) -> Self {
        let clean_default = strip_provider_prefix(&config.default_model);

        let openai_config = OpenAIConfig {
            api_key: config.api_key.clone(),
            base_url: OPENCODE_GO_BASE_URL.to_owned(),
            default_model: clean_default.clone(),
            timeout_secs: config.timeout_secs,
            max_retries: config.max_retries,
        };

        let messages_config = OpenCodeGoMessagesConfig {
            api_key: config.api_key,
            default_model: clean_default,
            timeout_secs: config.timeout_secs,
            max_retries: config.max_retries,
        };

        Self {
            inner_openai: OpenAIAdapter::new(openai_config),
            inner_messages: OpenCodeGoMessagesAdapter::new(messages_config),
        }
    }

    fn is_messages_model(model: &str) -> bool {
        let lower = model.to_lowercase();
        lower.contains("minimax")
    }
}

#[async_trait]
impl Provider for OpenCodeGoAdapter {
    fn name(&self) -> &str {
        "opencode-go"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: false,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: true,
            max_context_tokens: 1_000_000,
        }
    }

    async fn chat(&self, mut request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        request.model = strip_provider_prefix(&request.model);
        if Self::is_messages_model(&request.model) {
            self.inner_messages.chat(request).await
        } else {
            self.inner_openai.chat(request).await
        }
    }

    async fn chat_stream(
        &self,
        mut request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        request.model = strip_provider_prefix(&request.model);
        if Self::is_messages_model(&request.model) {
            self.inner_messages.chat_stream(request).await
        } else {
            self.inner_openai.chat_stream(request).await
        }
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let capabilities = self.capabilities();
        Ok(vec![
            ModelInfo {
                id: "opencode-go/glm-5.1".to_owned(),
                name: "GLM-5.1".to_owned(),
                context_window: Some(204_800),
                max_output_tokens: Some(131_072),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
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
                id: "opencode-go/glm-5".to_owned(),
                name: "GLM-5".to_owned(),
                context_window: Some(204_800),
                max_output_tokens: Some(131_072),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
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
                id: "opencode-go/kimi-k2.5".to_owned(),
                name: "Kimi K2.5".to_owned(),
                context_window: Some(256_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.6),
                    output: Some(3.0),
                    cached_input: Some(0.1),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/kimi-k2.6".to_owned(),
                name: "Kimi K2.6".to_owned(),
                context_window: Some(256_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.95),
                    output: Some(4.0),
                    cached_input: Some(0.16),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/mimo-v2-pro".to_owned(),
                name: "MiMo-V2-Pro".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: Some(32_768),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.5),
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
                id: "opencode-go/mimo-v2-omni".to_owned(),
                name: "MiMo-V2-Omni".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: Some(32_768),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.3),
                    output: Some(1.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/mimo-v2.5-pro".to_owned(),
                name: "MiMo-V2.5-Pro".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: Some(32_768),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.5),
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
                id: "opencode-go/mimo-v2.5".to_owned(),
                name: "MiMo-V2.5".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: Some(32_768),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.3),
                    output: Some(1.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/minimax-m2.7".to_owned(),
                name: "MiniMax M2.7".to_owned(),
                context_window: Some(256_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.3),
                    output: Some(1.2),
                    cached_input: Some(0.06),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/minimax-m2.5".to_owned(),
                name: "MiniMax M2.5".to_owned(),
                context_window: Some(256_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.3),
                    output: Some(1.2),
                    cached_input: Some(0.06),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/qwen3.6-plus".to_owned(),
                name: "Qwen3.6 Plus".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: Some(32_768),
                capabilities: capabilities.clone(),
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.5),
                    output: Some(3.0),
                    cached_input: Some(0.05),
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "opencode-go/qwen3.5-plus".to_owned(),
                name: "Qwen3.5 Plus".to_owned(),
                context_window: Some(128_000),
                max_output_tokens: Some(32_768),
                capabilities,
                pricing: Some(ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(0.2),
                    output: Some(1.2),
                    cached_input: Some(0.02),
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
        if let Err(e) = self.inner_openai.validate().await {
            tracing::warn!("OpenCode Go OpenAI validation failed: {}", e);
        }
        if let Err(e) = self.inner_messages.validate().await {
            tracing::warn!("OpenCode Go Messages validation failed: {}", e);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// OpenCode Go Messages Adapter (MiniMax via /v1/messages — Anthropic-compatible)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct OpenCodeGoMessagesConfig {
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

struct OpenCodeGoMessagesAdapter {
    config: OpenCodeGoMessagesConfig,
    client: reqwest::Client,
}

impl OpenCodeGoMessagesAdapter {
    fn new(config: OpenCodeGoMessagesConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("falha ao construir HTTP client");

        Self { config, client }
    }

    fn auth_headers(&self) -> anyhow::Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.config.api_key).parse()?,
        );
        headers.insert(reqwest::header::CONTENT_TYPE, "application/json".parse()?);
        Ok(headers)
    }
}

#[async_trait]
impl Provider for OpenCodeGoMessagesAdapter {
    fn name(&self) -> &str {
        "opencode-go-messages"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: false,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: true,
            max_context_tokens: 256_000,
        }
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = format!("{}/messages", OPENCODE_GO_BASE_URL);
        let body = build_messages_request(request);

        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers().map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Http { status, message: text });
        }

        let msg_resp: MessagesResponse =
            response.json().await.map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        Ok(msg_resp.into_chat_response())
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        let url = format!("{}/messages", OPENCODE_GO_BASE_URL);
        let mut body = build_messages_request(request);
        body.stream = true;

        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers().map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(ProviderError::Http { status, message: text });
        }

        let byte_stream = response.bytes_stream();
        let stream = byte_stream.flat_map(|result| {
            let items: Vec<Result<ChatChunk, ProviderError>> = match result {
                Ok(bytes) => parse_messages_sse_bytes(bytes),
                Err(e) => vec![Err(ProviderError::Http {
                    status: 0,
                    message: e.to_string(),
                })],
            };
            futures::stream::iter(items)
        });

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        if self.config.api_key.is_empty() {
            return Err(ProviderError::InvalidCredentials("opencode-go".to_owned()));
        }
        let url = format!("{}/messages", OPENCODE_GO_BASE_URL);
        let body = serde_json::json!({
            "model": self.config.default_model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "hi"}],
        });

        let response = self
            .client
            .post(&url)
            .headers(self.auth_headers().map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            Err(ProviderError::Http { status, message: text })
        }
    }
}

// ---------------------------------------------------------------------------
// Request / Response conversion
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<MessagesMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessagesMessage {
    role: String,
    content: String,
}

fn build_messages_request(req: ChatRequest) -> MessagesRequest {
    let mut system = None;
    let mut messages = Vec::new();

    for m in req.messages {
        match m {
            Message::System { content } => {
                system = Some(content);
            }
            Message::User { content } => {
                let text = match content {
                    MessageContent::Text(t) => t,
                    MessageContent::Parts(parts) => parts
                        .into_iter()
                        .filter_map(|p| match p {
                            hyscode_core::models::message::ContentPart::Text { text } => {
                                Some(text)
                            }
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                messages.push(MessagesMessage {
                    role: "user".to_owned(),
                    content: text,
                });
            }
            Message::Assistant { content, .. } => {
                messages.push(MessagesMessage {
                    role: "assistant".to_owned(),
                    content: content.unwrap_or_default(),
                });
            }
            Message::Tool { content, .. } => {
                messages.push(MessagesMessage {
                    role: "user".to_owned(),
                    content: format!("[tool result] {}", content),
                });
            }
        }
    }

    MessagesRequest {
        model: req.model,
        max_tokens: req.max_tokens.unwrap_or(4096),
        messages,
        system,
        stream: false,
        temperature: req.temperature,
        top_p: req.top_p,
        stop_sequences: req.stop,
    }
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    #[allow(dead_code)]
    role: String,
    content: Vec<MessagesContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: MessagesUsage,
}

#[derive(Debug, Deserialize)]
struct MessagesContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessagesUsage {
    input_tokens: u32,
    output_tokens: u32,
}

impl MessagesResponse {
    fn into_chat_response(self) -> ChatResponse {
        let content = self
            .content
            .into_iter()
            .filter_map(|block| {
                if block.block_type == "text" {
                    block.text
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let finish_reason = self
            .stop_reason
            .map(|r| match r.as_str() {
                "end_turn" => FinishReason::Stop,
                "max_tokens" => FinishReason::Length,
                "tool_use" => FinishReason::ToolCalls,
                "content_filter" => FinishReason::ContentFilter,
                _ => FinishReason::Error,
            })
            .unwrap_or(FinishReason::Stop);

        ChatResponse {
            id: self.id,
            model: self.model,
            content: Some(content).filter(|s| !s.is_empty()),
            tool_calls: None,
            finish_reason,
            usage: TokenUsage {
                prompt_tokens: self.usage.input_tokens,
                completion_tokens: self.usage.output_tokens,
                total_tokens: self.usage.input_tokens + self.usage.output_tokens,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// SSE parsing (Anthropic-compatible)
// ---------------------------------------------------------------------------

fn parse_messages_sse_bytes(bytes: Bytes) -> Vec<Result<ChatChunk, ProviderError>> {
    let text = String::from_utf8_lossy(&bytes);
    let mut current_event = String::new();
    let mut current_data = String::new();
    let mut chunks = Vec::new();

    for line in text.lines() {
        if line.starts_with("event: ") {
            current_event = line.strip_prefix("event: ").unwrap_or("").to_owned();
        } else if line.starts_with("data: ") {
            current_data = line.strip_prefix("data: ").unwrap_or("").to_owned();
        } else if line.is_empty() && !current_event.is_empty() {
            chunks.push(parse_messages_event(&current_event, &current_data));
            current_event.clear();
            current_data.clear();
        }
    }

    if !current_event.is_empty() {
        chunks.push(parse_messages_event(&current_event, &current_data));
    }

    if chunks.is_empty() {
        if let Some(data) = text.lines().find(|l| l.starts_with("data: ")) {
            let data = data.strip_prefix("data: ").unwrap_or(data);
            chunks.push(parse_messages_data_line(data));
        }
    }

    if chunks.is_empty() {
        chunks.push(Ok(ChatChunk {
            id: String::new(),
            delta: Delta::default(),
            finish_reason: None,
            usage: None,
        }));
    }

    chunks
}

fn parse_messages_event(event: &str, data: &str) -> Result<ChatChunk, ProviderError> {
    match event {
        "message_start" => {
            if let Ok(start) = serde_json::from_str::<MessagesMessageStart>(data) {
                return Ok(ChatChunk {
                    id: start.message.id,
                    delta: Delta::default(),
                    finish_reason: None,
                    usage: None,
                });
            }
        }
        "content_block_delta" => {
            if let Ok(delta) = serde_json::from_str::<MessagesContentBlockDelta>(data) {
                let content = match delta.delta {
                    MessagesDeltaInner::TextDelta { text } => Some(text),
                    _ => None,
                };
                return Ok(ChatChunk {
                    id: String::new(),
                    delta: Delta {
                        role: None,
                        content,
                        tool_call_delta: None,
                    },
                    finish_reason: None,
                    usage: None,
                });
            }
        }
        "message_delta" => {
            if let Ok(msg_delta) = serde_json::from_str::<MessagesMessageDelta>(data) {
                let finish_reason = msg_delta.delta.stop_reason.map(|r| match r.as_str() {
                    "end_turn" => FinishReason::Stop,
                    "max_tokens" => FinishReason::Length,
                    "tool_use" => FinishReason::ToolCalls,
                    "content_filter" => FinishReason::ContentFilter,
                    _ => FinishReason::Error,
                });
                let usage = msg_delta.usage.map(|u| TokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: u.output_tokens,
                    total_tokens: u.output_tokens,
                });
                return Ok(ChatChunk {
                    id: String::new(),
                    delta: Delta::default(),
                    finish_reason,
                    usage,
                });
            }
        }
        "message_stop" => {
            return Ok(ChatChunk {
                id: String::new(),
                delta: Delta::default(),
                finish_reason: Some(FinishReason::Stop),
                usage: None,
            });
        }
        "content_block_stop" | "content_block_start" | "ping" => {
            return Ok(ChatChunk {
                id: String::new(),
                delta: Delta::default(),
                finish_reason: None,
                usage: None,
            });
        }
        _ => {}
    }

    Ok(ChatChunk {
        id: String::new(),
        delta: Delta::default(),
        finish_reason: None,
        usage: None,
    })
}

fn parse_messages_data_line(data: &str) -> Result<ChatChunk, ProviderError> {
    if let Ok(start) = serde_json::from_str::<MessagesMessageStart>(data) {
        return Ok(ChatChunk {
            id: start.message.id,
            delta: Delta::default(),
            finish_reason: None,
            usage: None,
        });
    }
    if let Ok(delta) = serde_json::from_str::<MessagesContentBlockDelta>(data) {
        let content = match delta.delta {
            MessagesDeltaInner::TextDelta { text } => Some(text),
            _ => None,
        };
        return Ok(ChatChunk {
            id: String::new(),
            delta: Delta {
                role: None,
                content,
                tool_call_delta: None,
            },
            finish_reason: None,
            usage: None,
        });
    }
    if let Ok(msg_delta) = serde_json::from_str::<MessagesMessageDelta>(data) {
        let finish_reason = msg_delta.delta.stop_reason.map(|r| match r.as_str() {
            "end_turn" => FinishReason::Stop,
            "max_tokens" => FinishReason::Length,
            "tool_use" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Error,
        });
        return Ok(ChatChunk {
            id: String::new(),
            delta: Delta::default(),
            finish_reason,
            usage: None,
        });
    }

    Ok(ChatChunk {
        id: String::new(),
        delta: Delta::default(),
        finish_reason: None,
        usage: None,
    })
}

// Streaming types
#[derive(Debug, Deserialize)]
struct MessagesMessageStart {
    message: MessagesMessageStartInner,
}

#[derive(Debug, Deserialize)]
struct MessagesMessageStartInner {
    id: String,
}

#[derive(Debug, Deserialize)]
struct MessagesContentBlockDelta {
    #[allow(dead_code)]
    index: usize,
    delta: MessagesDeltaInner,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MessagesDeltaInner {
    TextDelta { text: String },
    #[allow(dead_code)]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
struct MessagesMessageDelta {
    delta: MessagesStopDelta,
    #[serde(default)]
    usage: Option<MessagesUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct MessagesStopDelta {
    stop_reason: Option<String>,
    #[allow(dead_code)]
    stop_sequence: Option<String>,
}
