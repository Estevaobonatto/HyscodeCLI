//! Adapter para o Hyscode Provider Service (SaaS próprio).
//!
//! Utiliza schema OpenAI-compatible, portanto reutiliza a mesma
//! estrutura de request/response do OpenAIAdapter.
//! Header de autenticação: `Authorization: Bearer hsk_...`

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{BoxStream, StreamExt};
use hyscode_core::{
    error::ProviderError,
    models::{
        message::{Message, MessageContent},
        provider::{ModelInfo, ProviderCapabilities},
        request::ChatRequest,
        response::{ChatChunk, ChatResponse, Delta, FinishReason, ToolCallDelta},
        usage::TokenUsage,
    },
    traits::provider::Provider,
};
use serde::{Deserialize, Serialize};

const HYSCODE_API_BASE: &str = "https://api.hyscode.dev/v1";

/// Configuração do adapter Hyscode Provider.
#[derive(Debug, Clone)]
pub struct HyscodeProviderConfig {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for HyscodeProviderConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: HYSCODE_API_BASE.to_owned(),
            default_model: "hyscode-smart".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para o Hyscode Provider Service.
///
/// Compatível com OpenAI API — usa os mesmos tipos de request/response,
/// mas aponta para o endpoint próprio e usa chave `hsk_...`.
pub struct HyscodeProviderAdapter {
    config: HyscodeProviderConfig,
    client: reqwest::Client,
}

impl HyscodeProviderAdapter {
    pub fn new(config: HyscodeProviderConfig) -> Self {
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
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse()?,
        );
        Ok(headers)
    }
}

#[async_trait]
impl Provider for HyscodeProviderAdapter {
    fn name(&self) -> &str {
        "hyscode"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // O serviço suporta todos os recursos, pois roteia para modelos capazes.
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
        let url = format!("{}/chat/completions", self.config.base_url);
        let body = HyscodeChatRequest::from_chat_request(request);

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

        let hyscode_resp: HyscodeChatResponse = response.json().await.map_err(|e| {
            ProviderError::Http {
                status: 0,
                message: e.to_string(),
            }
        })?;

        Ok(hyscode_resp.into_chat_response())
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url);
        let mut body = HyscodeChatRequest::from_chat_request(request);
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
                Ok(bytes) => Some(parse_hyscode_sse_bytes(bytes)),
                Err(e) => Some(Err(ProviderError::Http {
                    status: 0,
                    message: e.to_string(),
                })),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let url = format!("{}/models", self.config.base_url);
        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers().map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?)
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

        let models_resp: HyscodeModelsResponse = response.json().await.map_err(|e| {
            ProviderError::Http {
                status: 0,
                message: e.to_string(),
            }
        })?;

        Ok(models_resp
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.id,
                context_window: 200_000,
                capabilities: self.capabilities(),
            })
            .collect())
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        if self.config.api_key.is_empty() {
            return Err(ProviderError::InvalidCredentials("hyscode".to_owned()));
        }
        // GET /account para verificar credenciais e plano.
        let url = format!("{}/account", self.config.base_url);
        let response = self
            .client
            .get(&url)
            .headers(self.auth_headers().map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?)
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

fn parse_hyscode_sse_bytes(bytes: Bytes) -> Result<ChatChunk, ProviderError> {
    let text = String::from_utf8_lossy(&bytes);
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                return Ok(ChatChunk {
                    id: String::new(),
                    delta: Delta::default(),
                    finish_reason: Some(FinishReason::Stop),
                    usage: None,
                });
            }
            if let Ok(event) = serde_json::from_str::<HyscodeStreamEvent>(data) {
                return Ok(event.into_chat_chunk());
            }
        }
    }
    Ok(ChatChunk {
        id: String::new(),
        delta: Delta::default(),
        finish_reason: None,
        usage: None,
    })
}

// ---------------------------------------------------------------------------
// Tipos de request/response da API Hyscode (OpenAI-compatible)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct HyscodeChatRequest {
    model: String,
    messages: Vec<HyscodeMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct HyscodeMessage {
    role: String,
    content: String,
}

impl HyscodeChatRequest {
    fn from_chat_request(req: ChatRequest) -> Self {
        let messages = req
            .messages
            .into_iter()
            .map(|m| {
                let (role, content) = match m {
                    Message::System { content } => ("system".to_owned(), content),
                    Message::User { content } => {
                        let text = match content {
                            MessageContent::Text(t) => t,
                            MessageContent::Parts(parts) => parts
                                .into_iter()
                                .filter_map(|p| match p {
                                    hyscode_core::models::message::ContentPart::Text {
                                        text,
                                    } => Some(text),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n"),
                        };
                        ("user".to_owned(), text)
                    }
                    Message::Assistant { content, .. } => {
                        ("assistant".to_owned(), content.unwrap_or_default())
                    }
                    Message::Tool { content, .. } => ("tool".to_owned(), content),
                };
                HyscodeMessage { role, content }
            })
            .collect();

        Self {
            model: req.model,
            messages,
            stream: false,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            top_p: req.top_p,
            stop: req.stop,
        }
    }
}

#[derive(Debug, Deserialize)]
struct HyscodeChatResponse {
    id: String,
    model: String,
    choices: Vec<HyscodeChoice>,
    usage: Option<HyscodeUsage>,
}

#[derive(Debug, Deserialize)]
struct HyscodeChoice {
    message: HyscodeMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HyscodeUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct HyscodeStreamEvent {
    id: String,
    choices: Vec<HyscodeStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct HyscodeStreamChoice {
    delta: HyscodeStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct HyscodeStreamDelta {
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HyscodeModelsResponse {
    data: Vec<HyscodeModel>,
}

#[derive(Debug, Deserialize)]
struct HyscodeModel {
    id: String,
}

impl HyscodeChatResponse {
    fn into_chat_response(self) -> ChatResponse {
        let choice = self.choices.into_iter().next();
        let content = choice.as_ref().map(|c| c.message.content.clone());
        let finish_reason = choice
            .and_then(|c| c.finish_reason)
            .map(|r| match r.as_str() {
                "stop" => FinishReason::Stop,
                "length" => FinishReason::Length,
                "tool_calls" => FinishReason::ToolCalls,
                "content_filter" => FinishReason::ContentFilter,
                _ => FinishReason::Error,
            })
            .unwrap_or(FinishReason::Stop);

        let usage = self
            .usage
            .map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            })
            .unwrap_or_default();

        ChatResponse {
            id: self.id,
            model: self.model,
            content,
            tool_calls: None,
            finish_reason,
            usage,
        }
    }
}

impl HyscodeStreamEvent {
    fn into_chat_chunk(self) -> ChatChunk {
        let choice = self.choices.into_iter().next();
        let delta = choice
            .as_ref()
            .map(|c| Delta {
                role: c.delta.role.clone(),
                content: c.delta.content.clone(),
                tool_call_delta: None,
            })
            .unwrap_or_default();

        let finish_reason = choice.and_then(|c| c.finish_reason).and_then(|r| {
            match r.as_str() {
                "stop" => Some(FinishReason::Stop),
                "length" => Some(FinishReason::Length),
                "tool_calls" => Some(FinishReason::ToolCalls),
                "content_filter" => Some(FinishReason::ContentFilter),
                _ => Some(FinishReason::Error),
            }
        });

        ChatChunk {
            id: self.id,
            delta,
            finish_reason,
            usage: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hyscode_chat_request_from_chat_request() {
        let req = ChatRequest::new(
            "hyscode-smart".to_owned(),
            vec![
                Message::System {
                    content: "S".to_owned(),
                },
                Message::User {
                    content: MessageContent::Text("U".to_owned()),
                },
            ],
        );
        let hysc = HyscodeChatRequest::from_chat_request(req);
        assert_eq!(hysc.model, "hyscode-smart");
        assert_eq!(hysc.messages.len(), 2);
        assert_eq!(hysc.messages[0].role, "system");
        assert_eq!(hysc.messages[1].role, "user");
    }

    #[test]
    fn test_hyscode_response_into_chat_response() {
        let resp = HyscodeChatResponse {
            id: "chatcmpl-123".to_owned(),
            model: "hyscode-fast".to_owned(),
            choices: vec![HyscodeChoice {
                message: HyscodeMessage {
                    role: "assistant".to_owned(),
                    content: "Hello".to_owned(),
                },
                finish_reason: Some("stop".to_owned()),
            }],
            usage: Some(HyscodeUsage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            }),
        };
        let chat = resp.into_chat_response();
        assert_eq!(chat.id, "chatcmpl-123");
        assert_eq!(chat.content, Some("Hello".to_owned()));
        assert_eq!(chat.usage.total_tokens, 7);
    }

    #[test]
    fn test_parse_hyscode_sse_done() {
        let bytes = Bytes::from("data: [DONE]\n\n");
        let chunk = parse_hyscode_sse_bytes(bytes).unwrap();
        assert_eq!(chunk.finish_reason, Some(FinishReason::Stop));
    }
}
