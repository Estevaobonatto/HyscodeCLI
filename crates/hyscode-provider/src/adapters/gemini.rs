//! Adapter para a Google Gemini API.
//!
//! Suporta modelos Gemini 2.x/3.x via Google AI Studio (REST).
//! Protocolo: HTTPS + Server-Sent Events para streaming.
//! Referência: https://ai.google.dev/api/rest

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{BoxStream, StreamExt};
use hyscode_core::{
    error::ProviderError,
    models::{
        message::{ContentPart, Message, MessageContent},
        provider::{ModelInfo, ProviderCapabilities},
        request::ChatRequest,
        response::{ChatChunk, ChatResponse, Delta, FinishReason},
        tool::ToolCall,
        usage::TokenUsage,
    },
    traits::provider::Provider,
};
use serde::Deserialize;

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Configuração do adapter Gemini.
#[derive(Debug, Clone)]
pub struct GeminiConfig {
    pub api_key: String,
    pub base_url: String,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

impl Default for GeminiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: GEMINI_API_BASE.to_owned(),
            default_model: "gemini-2.5-flash".to_owned(),
            timeout_secs: 120,
            max_retries: 3,
        }
    }
}

/// Adapter para a API do Google Gemini.
pub struct GeminiAdapter {
    config: GeminiConfig,
    client: reqwest::Client,
}

impl GeminiAdapter {
    pub fn new(config: GeminiConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(config.timeout_secs))
            .build()
            .expect("falha ao construir HTTP client");

        Self { config, client }
    }

    fn auth_headers(&self) -> anyhow::Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("x-goog-api-key", self.config.api_key.parse()?);
        headers.insert(reqwest::header::CONTENT_TYPE, "application/json".parse()?);
        Ok(headers)
    }

    fn model_path(&self, model: &str) -> String {
        if model.starts_with("models/") {
            model.to_owned()
        } else {
            format!("models/{}", model)
        }
    }

    fn default_capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_vision: true,
            supports_streaming: true,
            supports_system_prompt: true,
            supports_parallel_tool_calls: true,
            max_context_tokens: 1_048_576,
        }
    }
}

#[async_trait]
impl Provider for GeminiAdapter {
    fn name(&self) -> &str {
        "gemini"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        Self::default_capabilities()
    }

    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let url = format!(
            "{}/{}:generateContent",
            self.config.base_url,
            self.model_path(&request.model)
        );
        let body = build_gemini_request(request);

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

        let gemini_resp: GeminiGenerateContentResponse =
            response.json().await.map_err(|e| ProviderError::Http {
                status: 0,
                message: e.to_string(),
            })?;

        Ok(gemini_resp.into_chat_response())
    }

    async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        let url = format!(
            "{}/{}:streamGenerateContent?alt=sse",
            self.config.base_url,
            self.model_path(&request.model)
        );
        let body = build_gemini_request(request);

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
        let stream = byte_stream.flat_map(|result| {
            let items: Vec<Result<ChatChunk, ProviderError>> = match result {
                Ok(bytes) => parse_gemini_sse_bytes(bytes),
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

        let capabilities = Self::default_capabilities();
        let mut models = known_gemini_models(capabilities.clone());

        if response.status().is_success() {
            if let Ok(list) = response.json::<GeminiListModelsResponse>().await {
                for m in list.models {
                    if !models.iter().any(|existing| existing.id == m.name) {
                        models.push(ModelInfo {
                            id: m.name.clone(),
                            name: m.display_name.unwrap_or_else(|| m.name.clone()),
                            context_window: Some(1_048_576),
                            max_output_tokens: None,
                            capabilities: capabilities.clone(),
                            pricing: None,
                        });
                    }
                }
            }
        }

        Ok(models)
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        if self.config.api_key.is_empty() {
            return Err(ProviderError::InvalidCredentials("gemini".to_owned()));
        }
        let url = format!(
            "{}/{}:generateContent",
            self.config.base_url,
            self.model_path(&self.config.default_model)
        );
        let body = serde_json::json!({
            "contents": [{"role": "user", "parts": [{"text": "hi"}]}],
            "generationConfig": {"maxOutputTokens": 1}
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

fn build_gemini_request(req: ChatRequest) -> serde_json::Value {
    let mut contents = Vec::new();
    let mut system_instruction = None;

    for m in req.messages {
        match m {
            Message::System { content } => {
                system_instruction = Some(serde_json::json!({
                    "parts": [{"text": content}]
                }));
            }
            Message::User { content } => {
                let parts = message_content_to_gemini_parts(content);
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": parts,
                }));
            }
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => {
                let mut parts = Vec::new();
                if let Some(text) = content {
                    parts.push(serde_json::json!({"text": text}));
                }
                if let Some(tcs) = tool_calls {
                    for tc in tcs {
                        let args: serde_json::Value =
                            serde_json::from_str(&tc.arguments).unwrap_or_default();
                        parts.push(serde_json::json!({
                            "functionCall": {
                                "name": tc.name,
                                "args": args,
                            }
                        }));
                    }
                }
                contents.push(serde_json::json!({
                    "role": "model",
                    "parts": parts,
                }));
            }
            Message::Tool {
                tool_call_id: _,
                content,
                is_error,
            } => {
                // Gemini não usa tool_call_id; mapeamos como functionResponse.
                // Precisamos do nome da função, mas o core não o passa no Message::Tool.
                // Usamos um wrapper genérico.
                let response_key = if is_error { "error" } else { "result" };
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{
                        "functionResponse": {
                            "response": {
                                response_key: content,
                            }
                        }
                    }],
                }));
            }
        }
    }

    let mut body = serde_json::json!({
        "contents": contents,
    });

    if let Some(si) = system_instruction {
        body["systemInstruction"] = si;
    }

    let mut generation_config = serde_json::Map::new();
    if let Some(t) = req.temperature {
        generation_config.insert("temperature".to_owned(), serde_json::json!(t));
    }
    if let Some(max) = req.max_tokens {
        generation_config.insert("maxOutputTokens".to_owned(), serde_json::json!(max));
    }
    if let Some(top_p) = req.top_p {
        generation_config.insert("topP".to_owned(), serde_json::json!(top_p));
    }
    if let Some(stop) = req.stop {
        generation_config.insert("stopSequences".to_owned(), serde_json::json!(stop));
    }
    if !generation_config.is_empty() {
        body["generationConfig"] = generation_config.into();
    }

    if let Some(tools) = req.tools {
        let function_declarations: Vec<_> = tools
            .into_iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                })
            })
            .collect();
        body["tools"] = serde_json::json!([{
            "functionDeclarations": function_declarations,
        }]);
    }

    body
}

fn message_content_to_gemini_parts(content: MessageContent) -> Vec<serde_json::Value> {
    match content {
        MessageContent::Text(text) => vec![serde_json::json!({"text": text})],
        MessageContent::Parts(parts) => parts
            .into_iter()
            .map(|p| match p {
                ContentPart::Text { text } => serde_json::json!({"text": text}),
                ContentPart::Image { source } => {
                    let mime = source.media_type;
                    match source.data {
                        hyscode_core::models::message::ImageData::Base64(data) => {
                            serde_json::json!({
                                "inlineData": {
                                    "mimeType": mime,
                                    "data": data,
                                }
                            })
                        }
                        hyscode_core::models::message::ImageData::Url(url) => {
                            // Gemini não suporta URL diretamente; fallback para descrição
                            serde_json::json!({"text": format!("[image: {}]", url)})
                        }
                    }
                }
            })
            .collect(),
    }
}

fn parse_gemini_sse_bytes(bytes: Bytes) -> Vec<Result<ChatChunk, ProviderError>> {
    let text = String::from_utf8_lossy(&bytes);
    let mut chunks = Vec::new();
    for line in text.lines() {
        if let Some(data) = line.strip_prefix("data: ") {
            if data.trim() == "[DONE]" {
                chunks.push(Ok(ChatChunk {
                    id: String::new(),
                    delta: Delta::default(),
                    finish_reason: Some(FinishReason::Stop),
                    usage: None,
                }));
                continue;
            }
            if let Ok(event) = serde_json::from_str::<GeminiGenerateContentResponse>(data) {
                chunks.push(Ok(event.into_chat_chunk()));
            }
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

fn known_gemini_models(capabilities: ProviderCapabilities) -> Vec<ModelInfo> {
    vec![
        // GA
        ModelInfo {
            id: "gemini-2.5-pro".to_owned(),
            name: "Gemini 2.5 Pro".to_owned(),
            context_window: Some(1_048_576),
            max_output_tokens: None,
            capabilities: capabilities.clone(),
            pricing: None,
        },
        ModelInfo {
            id: "gemini-2.5-flash".to_owned(),
            name: "Gemini 2.5 Flash".to_owned(),
            context_window: Some(1_048_576),
            max_output_tokens: None,
            capabilities: capabilities.clone(),
            pricing: None,
        },
        ModelInfo {
            id: "gemini-2.5-flash-lite".to_owned(),
            name: "Gemini 2.5 Flash-Lite".to_owned(),
            context_window: Some(1_048_576),
            max_output_tokens: None,
            capabilities: capabilities.clone(),
            pricing: None,
        },
        ModelInfo {
            id: "gemini-2.0-flash".to_owned(),
            name: "Gemini 2.0 Flash".to_owned(),
            context_window: Some(1_048_576),
            max_output_tokens: None,
            capabilities: capabilities.clone(),
            pricing: None,
        },
        ModelInfo {
            id: "gemini-2.0-flash-lite".to_owned(),
            name: "Gemini 2.0 Flash-Lite".to_owned(),
            context_window: Some(1_048_576),
            max_output_tokens: None,
            capabilities: capabilities.clone(),
            pricing: None,
        },
        // Preview / experimental
        ModelInfo {
            id: "gemini-3.1-pro-preview".to_owned(),
            name: "Gemini 3.1 Pro Preview".to_owned(),
            context_window: Some(1_000_000),
            max_output_tokens: Some(64_000),
            capabilities: capabilities.clone(),
            pricing: Some(hyscode_core::models::provider::ModelPricing {
                currency: "USD".to_owned(),
                unit: "per_1m_tokens".to_owned(),
                input: Some(2.0),
                output: Some(12.0),
                cached_input: None,
                long_context_input: Some(4.0),
                long_context_cached_input: None,
                long_context_output: Some(18.0),
                audio_input: None,
                image_output: None,
            }),
        },
        ModelInfo {
            id: "gemini-3.1-pro-preview-customtools".to_owned(),
            name: "Gemini 3.1 Pro Preview Custom Tools".to_owned(),
            context_window: Some(1_000_000),
            max_output_tokens: Some(64_000),
            capabilities: capabilities.clone(),
            pricing: Some(hyscode_core::models::provider::ModelPricing {
                currency: "USD".to_owned(),
                unit: "per_1m_tokens".to_owned(),
                input: Some(2.0),
                output: Some(12.0),
                cached_input: None,
                long_context_input: None,
                long_context_cached_input: None,
                long_context_output: None,
                audio_input: None,
                image_output: None,
            }),
        },
        ModelInfo {
            id: "gemini-3.1-flash-lite-preview".to_owned(),
            name: "Gemini 3.1 Flash-Lite Preview".to_owned(),
            context_window: Some(1_000_000),
            max_output_tokens: Some(64_000),
            capabilities: capabilities.clone(),
            pricing: Some(hyscode_core::models::provider::ModelPricing {
                currency: "USD".to_owned(),
                unit: "per_1m_tokens".to_owned(),
                input: Some(0.25),
                output: Some(1.5),
                cached_input: None,
                long_context_input: None,
                long_context_cached_input: None,
                long_context_output: None,
                audio_input: Some(0.5),
                image_output: None,
            }),
        },
        ModelInfo {
            id: "gemini-3-flash-preview".to_owned(),
            name: "Gemini 3 Flash Preview".to_owned(),
            context_window: Some(1_000_000),
            max_output_tokens: Some(64_000),
            capabilities: capabilities.clone(),
            pricing: Some(hyscode_core::models::provider::ModelPricing {
                currency: "USD".to_owned(),
                unit: "per_1m_tokens".to_owned(),
                input: Some(0.5),
                output: Some(3.0),
                cached_input: None,
                long_context_input: None,
                long_context_cached_input: None,
                long_context_output: None,
                audio_input: None,
                image_output: None,
            }),
        },
        ModelInfo {
            id: "gemini-3.1-flash-image-preview".to_owned(),
            name: "Gemini 3.1 Flash Image Preview".to_owned(),
            context_window: Some(128_000),
            max_output_tokens: Some(32_000),
            capabilities: capabilities.clone(),
            pricing: Some(hyscode_core::models::provider::ModelPricing {
                currency: "USD".to_owned(),
                unit: "mixed".to_owned(),
                input: Some(0.25),
                output: None,
                cached_input: None,
                long_context_input: None,
                long_context_cached_input: None,
                long_context_output: None,
                audio_input: None,
                image_output: Some(0.067),
            }),
        },
        ModelInfo {
            id: "gemini-3-pro-image-preview".to_owned(),
            name: "Gemini 3 Pro Image Preview".to_owned(),
            context_window: Some(65_000),
            max_output_tokens: Some(32_000),
            capabilities,
            pricing: Some(hyscode_core::models::provider::ModelPricing {
                currency: "USD".to_owned(),
                unit: "mixed".to_owned(),
                input: Some(2.0),
                output: None,
                cached_input: None,
                long_context_input: None,
                long_context_cached_input: None,
                long_context_output: None,
                audio_input: None,
                image_output: Some(0.134),
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Tipos da API Gemini
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiGenerateContentResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    #[serde(default)]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiCandidate {
    content: Option<GeminiContent>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiContent {
    #[serde(default)]
    parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiPart {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiFunctionCall {
    name: String,
    #[serde(default)]
    args: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsageMetadata {
    #[serde(default)]
    prompt_token_count: Option<u32>,
    #[serde(default)]
    candidates_token_count: Option<u32>,
    #[serde(default)]
    total_token_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiListModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiModel {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
}

impl GeminiGenerateContentResponse {
    fn into_chat_response(self) -> ChatResponse {
        let mut content_parts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut finish_reason = FinishReason::Stop;
        let model_name = String::new();

        if let Some(candidates) = self.candidates {
            if let Some(first) = candidates.into_iter().next() {
                if let Some(content) = first.content {
                    if let Some(parts) = content.parts {
                        for part in parts {
                            if let Some(text) = part.text {
                                content_parts.push(text);
                            }
                            if let Some(fc) = part.function_call {
                                let args = fc.args.unwrap_or_default();
                                tool_calls.push(ToolCall {
                                    id: format!("{}-{}", fc.name, tool_calls.len()),
                                    name: fc.name,
                                    arguments: args.to_string(),
                                });
                            }
                        }
                    }
                }
                finish_reason = match first.finish_reason.as_deref() {
                    Some("STOP") => FinishReason::Stop,
                    Some("MAX_TOKENS") => FinishReason::Length,
                    Some("SAFETY") => FinishReason::ContentFilter,
                    Some("RECITATION") => FinishReason::ContentFilter,
                    Some("OTHER") => FinishReason::Error,
                    _ => FinishReason::Stop,
                };
            }
        }

        let content = if content_parts.is_empty() {
            None
        } else {
            Some(content_parts.join(""))
        };

        let tool_calls = if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        };

        let usage = self
            .usage_metadata
            .map(|u| TokenUsage {
                prompt_tokens: u.prompt_token_count.unwrap_or(0),
                completion_tokens: u.candidates_token_count.unwrap_or(0),
                total_tokens: u.total_token_count.unwrap_or(0),
            })
            .unwrap_or_default();

        ChatResponse {
            id: String::new(),
            model: model_name,
            content,
            tool_calls,
            finish_reason,
            usage,
        }
    }

    fn into_chat_chunk(self) -> ChatChunk {
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        let mut finish_reason = None;

        if let Some(candidates) = self.candidates {
            if let Some(first) = candidates.into_iter().next() {
                if let Some(content) = first.content {
                    if let Some(parts) = content.parts {
                        for part in parts {
                            if let Some(text) = part.text {
                                text_parts.push(text);
                            }
                            if let Some(fc) = part.function_call {
                                let args = fc.args.unwrap_or_default();
                                tool_calls.push(ToolCall {
                                    id: format!("{}-{}", fc.name, tool_calls.len()),
                                    name: fc.name,
                                    arguments: args.to_string(),
                                });
                            }
                        }
                    }
                }
                finish_reason = match first.finish_reason.as_deref() {
                    Some("STOP") => Some(FinishReason::Stop),
                    Some("MAX_TOKENS") => Some(FinishReason::Length),
                    Some("SAFETY") => Some(FinishReason::ContentFilter),
                    Some("RECITATION") => Some(FinishReason::ContentFilter),
                    Some("OTHER") => Some(FinishReason::Error),
                    _ => None,
                };
            }
        }

        let content = if text_parts.is_empty() {
            None
        } else {
            Some(text_parts.join(""))
        };

        let tool_call_delta = if tool_calls.is_empty() {
            None
        } else {
            // Em streaming, Gemini retorna functionCall completo no chunk.
            // Mapeamos como tool_call_delta para o primeiro.
            tool_calls
                .into_iter()
                .next()
                .map(|tc| hyscode_core::models::response::ToolCallDelta {
                    index: 0,
                    id: Some(tc.id),
                    name: Some(tc.name),
                    arguments_chunk: Some(tc.arguments),
                })
        };

        let usage = self.usage_metadata.map(|u| TokenUsage {
            prompt_tokens: u.prompt_token_count.unwrap_or(0),
            completion_tokens: u.candidates_token_count.unwrap_or(0),
            total_tokens: u.total_token_count.unwrap_or(0),
        });

        ChatChunk {
            id: String::new(),
            delta: Delta {
                role: None,
                content,
                tool_call_delta,
            },
            finish_reason,
            usage,
        }
    }
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use hyscode_core::models::message::MessageContent;

    #[test]
    fn test_build_gemini_request_extracts_system() {
        let req = ChatRequest::new(
            "gemini-2.5-flash".to_owned(),
            vec![
                Message::System {
                    content: "Você é um assistente.".to_owned(),
                },
                Message::User {
                    content: MessageContent::Text("Olá".to_owned()),
                },
            ],
        );
        let body = build_gemini_request(req);
        assert!(body.get("systemInstruction").is_some());
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["role"], "user");
    }

    #[test]
    fn test_gemini_response_into_chat_response() {
        let resp = GeminiGenerateContentResponse {
            candidates: Some(vec![GeminiCandidate {
                content: Some(GeminiContent {
                    parts: Some(vec![GeminiPart {
                        text: Some("Resposta".to_owned()),
                        function_call: None,
                    }]),
                }),
                finish_reason: Some("STOP".to_owned()),
            }]),
            usage_metadata: Some(GeminiUsageMetadata {
                prompt_token_count: Some(10),
                candidates_token_count: Some(5),
                total_token_count: Some(15),
            }),
        };
        let chat = resp.into_chat_response();
        assert_eq!(chat.content, Some("Resposta".to_owned()));
        assert_eq!(chat.usage.total_tokens, 15);
        assert_eq!(chat.finish_reason, FinishReason::Stop);
    }

    #[test]
    fn test_parse_gemini_sse_text_delta() {
        let bytes = Bytes::from(
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}],\"role\":\"model\"},\"finishReason\":\"STOP\"}]}\n\n",
        );
        let chunk = parse_gemini_sse_bytes(bytes).unwrap();
        assert_eq!(chunk.delta.content, Some("Hello".to_owned()));
        assert_eq!(chunk.finish_reason, Some(FinishReason::Stop));
    }
}
