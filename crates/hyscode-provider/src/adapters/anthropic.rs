//! Adapter para a Anthropic Messages API.
//!
//! Suporta: claude-3-5-sonnet, claude-3-opus, claude-3-haiku e modelos futuros.
//! Protocolo: HTTPS + Server-Sent Events.
//! Referência: https://docs.anthropic.com/en/api/

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{BoxStream, StreamExt};
use hyscode_core::{
    error::ProviderError,
    models::{
        message::{Message, MessageContent},
        provider::{ModelInfo, ProviderCapabilities},
        request::ChatRequest,
        response::{ChatChunk, ChatResponse, Delta, FinishReason},
        usage::TokenUsage,
    },
    traits::provider::Provider,
};
use serde::{Deserialize, Serialize};

const ANTHROPIC_API_BASE: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Configuração do adapter Anthropic.
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            default_model: "claude-sonnet-4-6".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para a API da Anthropic.
pub struct AnthropicAdapter {
    config: AnthropicConfig,
    client: reqwest::Client,
}

impl AnthropicAdapter {
    pub fn new(config: AnthropicConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("falha ao construir HTTP client");

        Self { config, client }
    }

    fn auth_headers(&self) -> anyhow::Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-api-key", self.config.api_key.parse()?);
        headers.insert("anthropic-version", ANTHROPIC_VERSION.parse()?);
        headers.insert(reqwest::header::CONTENT_TYPE, "application/json".parse()?);
        Ok(headers)
    }
}

#[async_trait]
impl Provider for AnthropicAdapter {
    fn name(&self) -> &str {
        "anthropic"
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
        let url = format!("{}/messages", ANTHROPIC_API_BASE);
        let body = build_anthropic_request(request);

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
            return Err(ProviderError::Http {
                status,
                message: text,
            });
        }

        let anthropic_resp: AnthropicMessageResponse =
            response.json().await.map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        Ok(anthropic_resp.into_chat_response())
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        let url = format!("{}/messages", ANTHROPIC_API_BASE);
        let mut body = build_anthropic_request(request);
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
            return Err(ProviderError::Http {
                status,
                message: text,
            });
        }

        let byte_stream = response.bytes_stream();
        let stream = byte_stream.filter_map(|result| async move {
            match result {
                Ok(bytes) => Some(parse_anthropic_sse_bytes(bytes)),
                Err(e) => Some(Err(ProviderError::Http {
                    status: 0,
                    message: e.to_string(),
                })),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        // Anthropic não expõe endpoint público de listagem de modelos.
        // Retornamos lista hardcoded dos modelos conhecidos.
        let capabilities = self.capabilities();
        Ok(vec![
            ModelInfo {
                id: "claude-opus-4-7".to_owned(),
                name: "Claude Opus 4.7".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(128_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(5.0),
                    output: Some(25.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "claude-sonnet-4-6".to_owned(),
                name: "Claude Sonnet 4.6".to_owned(),
                context_window: Some(1_000_000),
                max_output_tokens: Some(64_000),
                capabilities: capabilities.clone(),
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(3.0),
                    output: Some(15.0),
                    cached_input: None,
                    long_context_input: None,
                    long_context_cached_input: None,
                    long_context_output: None,
                    audio_input: None,
                    image_output: None,
                }),
            },
            ModelInfo {
                id: "claude-haiku-4-5".to_owned(),
                name: "Claude Haiku 4.5".to_owned(),
                context_window: Some(200_000),
                max_output_tokens: Some(64_000),
                capabilities,
                pricing: Some(hyscode_core::models::provider::ModelPricing {
                    currency: "USD".to_owned(),
                    unit: "per_1m_tokens".to_owned(),
                    input: Some(1.0),
                    output: Some(5.0),
                    cached_input: None,
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
        if self.config.api_key.is_empty() {
            return Err(ProviderError::InvalidCredentials("anthropic".to_owned()));
        }
        // Faz uma requisição mínima para validar a chave.
        let url = format!("{}/messages", ANTHROPIC_API_BASE);
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
            Err(ProviderError::Http {
                status,
                message: text,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Conversão de request/response
// ---------------------------------------------------------------------------

fn build_anthropic_request(req: ChatRequest) -> AnthropicMessagesRequest {
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
                            hyscode_core::models::message::ContentPart::Text { text } => Some(text),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n"),
                };
                messages.push(AnthropicMessage {
                    role: "user".to_owned(),
                    content: text,
                });
            }
            Message::Assistant { content, .. } => {
                messages.push(AnthropicMessage {
                    role: "assistant".to_owned(),
                    content: content.unwrap_or_default(),
                });
            }
            Message::Tool { content, .. } => {
                // Anthropic usa role=user com content tipo tool_result,
                // mas para simplificar mapeamos como user message.
                messages.push(AnthropicMessage {
                    role: "user".to_owned(),
                    content: format!("[tool result] {}", content),
                });
            }
        }
    }

    AnthropicMessagesRequest {
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

fn parse_anthropic_sse_bytes(bytes: Bytes) -> Result<ChatChunk, ProviderError> {
    let text = String::from_utf8_lossy(&bytes);
    let mut current_event = String::new();
    let mut current_data = String::new();

    for line in text.lines() {
        if line.starts_with("event: ") {
            current_event = line.strip_prefix("event: ").unwrap_or("").to_owned();
        } else if line.starts_with("data: ") {
            current_data = line.strip_prefix("data: ").unwrap_or("").to_owned();
        } else if line.is_empty() && !current_event.is_empty() {
            // Processa o evento completo
            return parse_anthropic_event(&current_event, &current_data);
        }
    }

    // Se não houve evento vazio no final, processa o último acumulado
    if !current_event.is_empty() {
        return parse_anthropic_event(&current_event, &current_data);
    }

    // Linha simples sem event: (formato simplificado)
    if let Some(data) = text.lines().find(|l| l.starts_with("data: ")) {
        let data = data.strip_prefix("data: ").unwrap_or(data);
        return parse_anthropic_data_line(data);
    }

    Ok(ChatChunk {
        id: String::new(),
        delta: Delta::default(),
        finish_reason: None,
        usage: None,
    })
}

fn parse_anthropic_event(event: &str, data: &str) -> Result<ChatChunk, ProviderError> {
    match event {
        "message_start" => {
            if let Ok(start) = serde_json::from_str::<AnthropicMessageStart>(data) {
                return Ok(ChatChunk {
                    id: start.message.id,
                    delta: Delta::default(),
                    finish_reason: None,
                    usage: None,
                });
            }
        }
        "content_block_delta" => {
            if let Ok(delta) = serde_json::from_str::<AnthropicContentBlockDelta>(data) {
                let content = match delta.delta {
                    AnthropicDeltaInner::TextDelta { text } => Some(text),
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
            if let Ok(msg_delta) = serde_json::from_str::<AnthropicMessageDelta>(data) {
                let finish_reason = msg_delta.delta.stop_reason.map(|r| match r.as_str() {
                    "end_turn" => FinishReason::Stop,
                    "max_tokens" => FinishReason::Length,
                    "tool_use" => FinishReason::ToolCalls,
                    "content_filter" => FinishReason::ContentFilter,
                    _ => FinishReason::Error,
                });
                let usage = msg_delta.usage.map(|u| TokenUsage {
                    prompt_tokens: 0, // Anthropic não retorna input tokens no delta
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
            // Eventos de controle — retornam chunk vazio
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

fn parse_anthropic_data_line(data: &str) -> Result<ChatChunk, ProviderError> {
    // Tenta interpretar como evento inline (algumas libs não usam event:)
    if let Ok(start) = serde_json::from_str::<AnthropicMessageStart>(data) {
        return Ok(ChatChunk {
            id: start.message.id,
            delta: Delta::default(),
            finish_reason: None,
            usage: None,
        });
    }
    if let Ok(delta) = serde_json::from_str::<AnthropicContentBlockDelta>(data) {
        let content = match delta.delta {
            AnthropicDeltaInner::TextDelta { text } => Some(text),
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
    if let Ok(msg_delta) = serde_json::from_str::<AnthropicMessageDelta>(data) {
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

// ---------------------------------------------------------------------------
// Tipos da API Anthropic
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct AnthropicMessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
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
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageResponse {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    #[allow(dead_code)]
    role: String,
    content: Vec<AnthropicContentBlock>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

impl AnthropicMessageResponse {
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

// Streaming types
#[derive(Debug, Deserialize)]
struct AnthropicMessageStart {
    message: AnthropicMessageStartInner,
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageStartInner {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicContentBlockDelta {
    #[allow(dead_code)]
    index: usize,
    delta: AnthropicDeltaInner,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicDeltaInner {
    TextDelta {
        text: String,
    },
    #[allow(dead_code)]
    InputJsonDelta {
        partial_json: String,
    },
}

#[derive(Debug, Deserialize)]
struct AnthropicMessageDelta {
    delta: AnthropicStopDelta,
    #[serde(default)]
    usage: Option<AnthropicUsage>,
}

#[derive(Debug, Deserialize, Default)]
struct AnthropicStopDelta {
    stop_reason: Option<String>,
    #[allow(dead_code)]
    stop_sequence: Option<String>,
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_anthropic_request_extracts_system() {
        let req = ChatRequest::new(
            "claude-3-5-sonnet".to_owned(),
            vec![
                Message::System {
                    content: "Você é um assistente.".to_owned(),
                },
                Message::User {
                    content: MessageContent::Text("Olá".to_owned()),
                },
            ],
        );
        let anthropic_req = build_anthropic_request(req);
        assert_eq!(
            anthropic_req.system,
            Some("Você é um assistente.".to_owned())
        );
        assert_eq!(anthropic_req.messages.len(), 1);
        assert_eq!(anthropic_req.messages[0].role, "user");
        assert_eq!(anthropic_req.messages[0].content, "Olá");
    }

    #[test]
    fn test_parse_anthropic_sse_text_delta() {
        let bytes = Bytes::from(
            "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
        );
        let chunk = parse_anthropic_sse_bytes(bytes).unwrap();
        assert_eq!(chunk.delta.content, Some("Hello".to_owned()));
    }

    #[test]
    fn test_parse_anthropic_sse_message_stop() {
        let bytes = Bytes::from("event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n");
        let chunk = parse_anthropic_sse_bytes(bytes).unwrap();
        assert_eq!(chunk.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_anthropic_response_into_chat_response() {
        let anthropic_resp = AnthropicMessageResponse {
            id: "msg_123".to_owned(),
            _type: "message".to_owned(),
            role: "assistant".to_owned(),
            content: vec![AnthropicContentBlock {
                block_type: "text".to_owned(),
                text: Some("Resposta".to_owned()),
            }],
            model: "claude-3-5-sonnet".to_owned(),
            stop_reason: Some("end_turn".to_owned()),
            usage: AnthropicUsage {
                input_tokens: 10,
                output_tokens: 5,
            },
        };
        let chat = anthropic_resp.into_chat_response();
        assert_eq!(chat.id, "msg_123");
        assert_eq!(chat.content, Some("Resposta".to_owned()));
        assert_eq!(chat.usage.total_tokens, 15);
        assert_eq!(chat.finish_reason, FinishReason::Stop);
    }
}
