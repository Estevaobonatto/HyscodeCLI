//! Adapter para a OpenAI API.
//!
//! Suporta: gpt-4o, gpt-4-turbo, gpt-3.5-turbo e compatíveis.
//! Protocolo: HTTPS + Server-Sent Events.

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
        tool::ToolCall,
        usage::TokenUsage,
    },
    traits::provider::Provider,
};
use serde::{Deserialize, Serialize};

/// Configuração do adapter OpenAI.
#[derive(Debug, Clone)]
pub struct OpenAIConfig {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for OpenAIConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_owned(),
            default_model: "gpt-4o".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para a API da OpenAI.
pub struct OpenAIAdapter {
    config: OpenAIConfig,
    client: reqwest::Client,
}

impl OpenAIAdapter {
    pub fn new(config: OpenAIConfig) -> Self {
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
impl Provider for OpenAIAdapter {
    fn name(&self) -> &str {
        "openai"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: true,
            max_context_tokens: 128_000,
        }
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url);
        let body = OpenAIChatRequest::from_chat_request(request);

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

        let openai_resp: OpenAIChatResponse =
            response.json().await.map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        Ok(openai_resp.into_chat_response())
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        let url = format!("{}/chat/completions", self.config.base_url);
        let mut body = OpenAIChatRequest::from_chat_request(request);
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
                Ok(bytes) => Some(parse_sse_bytes(bytes)),
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

        let models_resp: OpenAIModelsResponse =
            response.json().await.map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        Ok(models_resp
            .data
            .into_iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.id,
                context_window: 128_000,
                capabilities: self.capabilities(),
            })
            .collect())
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        if self.config.api_key.is_empty() {
            return Err(ProviderError::InvalidCredentials("openai".to_owned()));
        }
        self.list_models().await?;
        Ok(())
    }
}

fn parse_sse_bytes(bytes: Bytes) -> Result<ChatChunk, ProviderError> {
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
            if let Ok(event) = serde_json::from_str::<OpenAIStreamEvent>(data) {
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
// Tipos de request/response da API OpenAI
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct OpenAIChatRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAIToolDefinitionRequest>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    function: OpenAIToolFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAIToolFunction {
    name: String,
    arguments: String,
}

/// Definição de ferramenta enviada ao modelo (formato da API OpenAI).
#[derive(Debug, Serialize)]
struct OpenAIToolDefinitionRequest {
    #[serde(rename = "type")]
    _type: String,
    function: OpenAIFunctionSpec,
}

#[derive(Debug, Serialize)]
struct OpenAIFunctionSpec {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

impl OpenAIChatRequest {
    fn from_chat_request(req: ChatRequest) -> Self {
        let messages = req
            .messages
            .into_iter()
            .map(|m| match m {
                Message::System { content } => OpenAIMessage {
                    role: "system".to_owned(),
                    content: Some(content),
                    tool_calls: None,
                    tool_call_id: None,
                },
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
                    OpenAIMessage {
                        role: "user".to_owned(),
                        content: Some(text),
                        tool_calls: None,
                        tool_call_id: None,
                    }
                }
                Message::Assistant {
                    content,
                    tool_calls,
                    ..
                } => {
                    let tcs = tool_calls.map(|tcs| {
                        tcs.into_iter()
                            .map(|tc| OpenAIToolCall {
                                id: tc.id,
                                _type: "function".to_owned(),
                                function: OpenAIToolFunction {
                                    name: tc.name,
                                    arguments: tc.arguments,
                                },
                            })
                            .collect()
                    });
                    OpenAIMessage {
                        role: "assistant".to_owned(),
                        content,
                        tool_calls: tcs,
                        tool_call_id: None,
                    }
                }
                Message::Tool {
                    tool_call_id,
                    content,
                    ..
                } => OpenAIMessage {
                    role: "tool".to_owned(),
                    content: Some(content),
                    tool_calls: None,
                    tool_call_id: Some(tool_call_id),
                },
            })
            .collect();

        let tools = req.tools.map(|tools| {
            tools
                .into_iter()
                .map(|t| OpenAIToolDefinitionRequest {
                    _type: "function".to_owned(),
                    function: OpenAIFunctionSpec {
                        name: t.name,
                        description: t.description,
                        parameters: t.parameters,
                    },
                })
                .collect()
        });

        Self {
            model: req.model,
            messages,
            stream: false,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            top_p: req.top_p,
            stop: req.stop,
            tools,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenAIChatResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamEvent {
    id: String,
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAIStreamDelta {
    role: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModelsResponse {
    data: Vec<OpenAIModel>,
}

#[derive(Debug, Deserialize)]
struct OpenAIModel {
    id: String,
}

impl OpenAIChatResponse {
    fn into_chat_response(self) -> ChatResponse {
        let choice = self.choices.into_iter().next();
        let content = choice.as_ref().and_then(|c| c.message.content.clone());
        let tool_calls = choice.as_ref().and_then(|c| {
            c.message.tool_calls.as_ref().map(|tcs| {
                tcs.iter()
                    .map(|tc| ToolCall {
                        id: tc.id.clone(),
                        name: tc.function.name.clone(),
                        arguments: tc.function.arguments.clone(),
                    })
                    .collect::<Vec<_>>()
            })
        });
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
            tool_calls,
            finish_reason,
            usage,
        }
    }
}

impl OpenAIStreamEvent {
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

        let finish_reason = choice
            .and_then(|c| c.finish_reason)
            .map(|r| match r.as_str() {
                "stop" => FinishReason::Stop,
                "length" => FinishReason::Length,
                "tool_calls" => FinishReason::ToolCalls,
                "content_filter" => FinishReason::ContentFilter,
                _ => FinishReason::Error,
            });

        ChatChunk {
            id: self.id,
            delta,
            finish_reason,
            usage: None,
        }
    }
}
