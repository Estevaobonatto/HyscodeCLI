//! Normalização entre formato OpenAI e Anthropic.
//!
//! Anthropic `/v1/messages` usa schema diferente de OpenAI `/v1/chat/completions`.
//! Este módulo converte request e response nos dois sentidos,
//! incluindo chunks SSE de streaming.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::error::{GatewayError, Result};
use crate::models::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, UsageInfo,
};

// ---------------------------------------------------------------------------
// Anthropic Request
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: String,
}

/// Converte request OpenAI → Anthropic.
pub fn openai_to_anthropic(req: &ChatCompletionRequest) -> AnthropicRequest {
    let mut system: Option<String> = None;
    let mut messages: Vec<AnthropicMessage> = Vec::with_capacity(req.messages.len());

    for msg in &req.messages {
        let content = message_content_str(&msg.content);
        if msg.role == "system" {
            // Anthropic separa system em campo próprio
            system = Some(content);
        } else {
            // mapeia "assistant" → "assistant", "user" → "user", outros → "user"
            let role = if msg.role == "assistant" {
                "assistant"
            } else {
                "user"
            };
            messages.push(AnthropicMessage {
                role: role.to_owned(),
                content,
            });
        }
    }

    AnthropicRequest {
        model: req.model.clone(),
        messages,
        system,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        stream: Some(req.stream),
    }
}

fn message_content_str(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Anthropic Response (non-streaming)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub id: String,
    #[allow(dead_code)]
    pub model: String,
    pub content: Vec<AnthropicContentBlock>,
    pub usage: AnthropicUsage,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Converte resposta Anthropic → OpenAI.
pub fn anthropic_to_openai(raw: &str, model: &str) -> Result<ChatCompletionResponse> {
    let resp: AnthropicResponse = serde_json::from_str(raw)
        .map_err(|e| GatewayError::UpstreamError(format!("parse anthropic: {}", e)))?;

    let text = resp
        .content
        .iter()
        .filter(|b| b.block_type == "text")
        .filter_map(|b| b.text.clone())
        .collect::<Vec<_>>()
        .join("");

    Ok(ChatCompletionResponse {
        id: resp.id,
        object: "chat.completion".to_owned(),
        created: chrono::Utc::now().timestamp(),
        model: model.to_owned(),
        choices: vec![Choice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_owned(),
                content: Value::String(text),
            },
            finish_reason: resp.stop_reason.unwrap_or_else(|| "stop".to_owned()),
        }],
        usage: UsageInfo {
            prompt_tokens: resp.usage.input_tokens,
            completion_tokens: resp.usage.output_tokens,
            total_tokens: resp.usage.input_tokens + resp.usage.output_tokens,
        },
    })
}

// ---------------------------------------------------------------------------
// Anthropic SSE streaming normalization
// ---------------------------------------------------------------------------

/// Converte um chunk SSE bruto do Anthropic em chunk SSE compatível OpenAI.
/// Retorna `None` para eventos que não precisam ser repassados (ex: ping).
pub fn normalize_anthropic_sse(chunk: &str) -> Option<String> {
    // Anthropic envia linhas como:
    // event: content_block_delta
    // data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Olá"}}
    //
    // OpenAI espera:
    // data: {"id":"...","object":"chat.completion.chunk","choices":[{"delta":{"content":"Olá"}}]}

    let mut event_type: Option<String> = None;
    let mut data_line: Option<String> = None;

    for line in chunk.lines() {
        if let Some(stripped) = line.strip_prefix("event:") {
            event_type = Some(stripped.trim().to_owned());
        } else if let Some(stripped) = line.strip_prefix("data:") {
            data_line = Some(stripped.trim().to_owned());
        }
    }

    let event = event_type.as_deref()?;
    let data = data_line?;

    match event {
        "content_block_delta" => {
            let parsed: Value = serde_json::from_str(&data).ok()?;
            let text = parsed["delta"]["text"].as_str()?;
            let openai_chunk = json!({
                "object": "chat.completion.chunk",
                "choices": [{
                    "index": 0,
                    "delta": { "content": text }
                }]
            });
            Some(format!("data: {}\n\n", openai_chunk))
        }
        "message_stop" => {
            let openai_chunk = json!({
                "object": "chat.completion.chunk",
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": "stop"
                }]
            });
            Some(format!("data: {}\n\n", openai_chunk))
        }
        "message_start" | "content_block_start" | "content_block_stop" | "ping" => {
            // Ignora ou mapeia para vazio
            None
        }
        _ => {
            // Evento desconhecido: repassa como-is (melhor esforço)
            Some(format!("data: {}\n\n", data))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_openai_to_anthropic_system_separated() {
        let req = ChatCompletionRequest {
            model: "claude-test".to_owned(),
            messages: vec![
                ChatMessage {
                    role: "system".to_owned(),
                    content: json!("Behave"),
                },
                ChatMessage {
                    role: "user".to_owned(),
                    content: json!("Hello"),
                },
            ],
            stream: false,
            temperature: None,
            max_tokens: Some(100),
        };
        let anth = openai_to_anthropic(&req);
        assert_eq!(anth.system, Some("Behave".to_owned()));
        assert_eq!(anth.messages.len(), 1);
        assert_eq!(anth.messages[0].role, "user");
        assert_eq!(anth.messages[0].content, "Hello");
        assert_eq!(anth.max_tokens, Some(100));
    }

    #[test]
    fn test_anthropic_to_openai_response() {
        let raw = r#"{
            "id": "msg_01",
            "model": "claude-3",
            "content": [{"type":"text","text":"Hi there"}],
            "usage": {"input_tokens": 10, "output_tokens": 5},
            "stop_reason": "end_turn"
        }"#;
        let resp = anthropic_to_openai(raw, "claude-alias").unwrap();
        assert_eq!(resp.id, "msg_01");
        assert_eq!(resp.model, "claude-alias");
        assert_eq!(resp.choices.len(), 1);
        assert_eq!(resp.choices[0].message.content, json!("Hi there"));
        assert_eq!(resp.usage.prompt_tokens, 10);
        assert_eq!(resp.usage.completion_tokens, 5);
        assert_eq!(resp.usage.total_tokens, 15);
    }

    #[test]
    fn test_normalize_anthropic_sse_delta() {
        let chunk = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Ol\"}}";
        let out = normalize_anthropic_sse(chunk).unwrap();
        assert!(out.contains("chat.completion.chunk"));
        assert!(out.contains("Ol"));
    }

    #[test]
    fn test_normalize_anthropic_sse_stop() {
        let chunk = "event: message_stop\ndata: {\"type\":\"message_stop\"}";
        let out = normalize_anthropic_sse(chunk).unwrap();
        assert!(out.contains("finish_reason\":\"stop\""));
    }

    #[test]
    fn test_normalize_anthropic_sse_ignored() {
        let chunk = "event: ping\ndata: {}";
        assert!(normalize_anthropic_sse(chunk).is_none());
    }
}
