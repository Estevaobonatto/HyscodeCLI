//! Upstream LLM proxy — encaminha requisições para o provedor correto.
//!
//! Suporta streaming via SSE (Server-Sent Events) e resposta completa.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Extension, State},
    response::{
        sse::{Event, Sse},
        IntoResponse, Json, Response,
    },
};
use futures::stream::Stream;
use reqwest::Client;
use serde_json::Value;
use tokio_stream::StreamExt;
use uuid::Uuid;

use crate::{
    anthropic,
    auth::AuthContext,
    billing,
    config::{Config, ModelRoute},
    error::{GatewayError, Result},
    models::{
        ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, HealthResponse,
        ModelInfo, ModelsResponse, UsageInfo,
    },
};

/// Estado compartilhado com o router axum.
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    #[allow(dead_code)]
    pub redis: redis::aio::ConnectionManager,
    pub config: Arc<Config>,
    pub http: Client,
}

impl AppState {
    pub fn new(db: sqlx::PgPool, redis: redis::aio::ConnectionManager, config: Config) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Falha ao criar HTTP client");

        Self {
            db,
            redis,
            config: Arc::new(config),
            http,
        }
    }

    /// Resolve o provedor para um model alias.
    pub fn resolve_provider(&self, model: &str) -> Option<&ModelRoute> {
        self.config.model_routes.iter().find(|r| r.model == model)
    }
}

/// GET /health
pub async fn health_handler() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_owned(),
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// GET /v1/models
pub async fn list_models_handler(
    Extension(_auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse> {
    let now = chrono::Utc::now().timestamp();
    let models: Vec<ModelInfo> = state
        .config
        .model_routes
        .iter()
        .map(|r| ModelInfo {
            id: r.model.clone(),
            object: "model".to_owned(),
            created: now,
            owned_by: r.provider.clone(),
        })
        .collect();

    Ok(Json(ModelsResponse {
        object: "list".to_owned(),
        data: models,
    }))
}

/// POST /v1/chat/completions
pub async fn chat_completions_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Response> {
    // Verifica quota antes de processar
    check_quota(&state, auth.user_id).await?;

    let route = state
        .resolve_provider(&req.model)
        .ok_or_else(|| GatewayError::ModelNotFound(req.model.clone()))?;

    let upstream_url = upstream_url(&route.provider);
    let api_key = provider_api_key(&route.provider)?;
    let is_anthropic = route.provider == "anthropic";

    if req.stream {
        let stream =
            stream_chat(&state, &auth, &req, &upstream_url, &api_key, is_anthropic).await?;
        Ok(Sse::new(stream).into_response())
    } else {
        let start = Instant::now();
        let response =
            forward_request(&state.http, &upstream_url, &api_key, &req, is_anthropic).await?;
        let latency = start.elapsed().as_millis() as i32;
        log_request(
            &state,
            &auth,
            &req.model,
            &route.provider,
            latency,
            Some(&response),
        )
        .await;
        Ok(Json(response).into_response())
    }
}

// ---------------------------------------------------------------------------
// Non-streaming forward
// ---------------------------------------------------------------------------

async fn forward_request(
    http: &Client,
    upstream_url: &str,
    api_key: &str,
    req: &ChatCompletionRequest,
    is_anthropic: bool,
) -> Result<ChatCompletionResponse> {
    let body: Value = if is_anthropic {
        serde_json::to_value(anthropic::openai_to_anthropic(req))
            .map_err(|e| GatewayError::Internal(e.into()))?
    } else {
        serde_json::to_value(req).map_err(|e| GatewayError::Internal(e.into()))?
    };

    let resp = http
        .post(upstream_url)
        .bearer_auth(api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| GatewayError::UpstreamError(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(GatewayError::UpstreamError(format!(
            "HTTP {}: {}",
            status, text
        )));
    }

    let raw_text = resp
        .text()
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    if is_anthropic {
        anthropic::anthropic_to_openai(&raw_text, &req.model)
    } else {
        parse_openai_response(&raw_text)
    }
}

fn parse_openai_response(raw: &str) -> Result<ChatCompletionResponse> {
    let raw: Value = serde_json::from_str(raw)
        .map_err(|e| GatewayError::UpstreamError(format!("parse openai: {}", e)))?;

    let usage = UsageInfo {
        prompt_tokens: raw["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
        completion_tokens: raw["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
        total_tokens: raw["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
    };

    let choice = raw["choices"][0].clone();
    let message = ChatMessage {
        role: choice["message"]["role"]
            .as_str()
            .unwrap_or("assistant")
            .to_owned(),
        content: choice["message"]["content"].clone(),
    };

    Ok(ChatCompletionResponse {
        id: raw["id"].as_str().unwrap_or("").to_owned(),
        object: "chat.completion".to_owned(),
        created: raw["created"].as_i64().unwrap_or_default(),
        model: raw["model"].as_str().unwrap_or("").to_owned(),
        choices: vec![Choice {
            index: 0,
            message,
            finish_reason: choice["finish_reason"]
                .as_str()
                .unwrap_or("stop")
                .to_owned(),
        }],
        usage,
    })
}

// ---------------------------------------------------------------------------
// Streaming forward
// ---------------------------------------------------------------------------

async fn stream_chat(
    state: &AppState,
    auth: &AuthContext,
    req: &ChatCompletionRequest,
    upstream_url: &str,
    api_key: &str,
    is_anthropic: bool,
) -> std::result::Result<impl Stream<Item = std::result::Result<Event, Infallible>>, GatewayError> {
    let body: Value = if is_anthropic {
        serde_json::to_value(anthropic::openai_to_anthropic(req))
            .map_err(|e| GatewayError::Internal(e.into()))?
    } else {
        serde_json::to_value(req).map_err(|e| GatewayError::Internal(e.into()))?
    };

    let resp = state
        .http
        .post(upstream_url)
        .bearer_auth(api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .map_err(|e| GatewayError::UpstreamError(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(GatewayError::UpstreamError(format!(
            "HTTP {}: {}",
            status, text
        )));
    }

    let model = req.model.clone();
    let provider = if is_anthropic { "anthropic" } else { "openai" }.to_owned();
    let api_key_id = auth.api_key_id;
    let user_id = auth.user_id;
    let db = state.db.clone();

    // Fire-and-forget logging de request streaming (sem token count exato)
    tokio::spawn(async move {
        let _ = sqlx::query(
            r#"
            INSERT INTO requests_log
                (api_key_id, user_id, model, upstream_provider, prompt_tokens, completion_tokens, total_tokens, status_code)
            VALUES ($1, $2, $3, $4, 0, 0, 0, 200)
            "#,
        )
        .bind(api_key_id)
        .bind(user_id)
        .bind(model)
        .bind(provider)
        .execute(&db)
        .await;
    });

    let bytes_stream = resp.bytes_stream();

    let event_stream = bytes_stream.filter_map(move |result| {
        let chunk = match result {
            Ok(c) => c,
            Err(_) => return Some(Ok(Event::default().data("[stream error]"))),
        };

        let text = String::from_utf8_lossy(&chunk);

        if is_anthropic {
            // Anthropic envia blocos SSE que precisam de normalização
            anthropic::normalize_anthropic_sse(&text)
                .map(|normalized| Ok(Event::default().data(normalized)))
        } else {
            // OpenAI já é compatível: repassa como-is (preservando prefixo `data:`)
            Some(Ok(Event::default().data(text.as_ref())))
        }
    });

    Ok(event_stream)
}

// ---------------------------------------------------------------------------
// Quota
// ---------------------------------------------------------------------------

async fn check_quota(state: &AppState, user_id: Uuid) -> Result<()> {
    let row: Option<(i64, i64)> =
        sqlx::query_as("SELECT monthly_tokens, monthly_limit FROM usage_quotas WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
            .map_err(|e| GatewayError::Internal(e.into()))?;

    if let Some((used, limit)) = row {
        if used >= limit {
            return Err(GatewayError::QuotaExceeded);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

async fn log_request(
    state: &AppState,
    auth: &AuthContext,
    model: &str,
    provider: &str,
    latency_ms: i32,
    resp: Option<&ChatCompletionResponse>,
) {
    let (prompt_tokens, completion_tokens, total_tokens) = match resp {
        Some(r) => (
            r.usage.prompt_tokens as i32,
            r.usage.completion_tokens as i32,
            r.usage.total_tokens as i32,
        ),
        None => (0, 0, 0),
    };

    let _ = sqlx::query(
        r#"
        INSERT INTO requests_log
            (api_key_id, user_id, model, upstream_provider, prompt_tokens, completion_tokens, total_tokens, status_code, latency_ms)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 200, $8)
        "#,
    )
    .bind(auth.api_key_id)
    .bind(auth.user_id)
    .bind(model)
    .bind(provider)
    .bind(prompt_tokens)
    .bind(completion_tokens)
    .bind(total_tokens)
    .bind(latency_ms)
    .execute(&state.db)
    .await;

    if total_tokens > 0 {
        let _ = sqlx::query(
            r#"
            INSERT INTO usage_quotas (user_id, monthly_tokens)
            VALUES ($1, $2)
            ON CONFLICT (user_id) DO UPDATE
                SET monthly_tokens = usage_quotas.monthly_tokens + $2,
                    updated_at = NOW()
            "#,
        )
        .bind(auth.user_id)
        .bind(total_tokens as i64)
        .execute(&state.db)
        .await;

        // Fase 5: cobrança por modelo
        let _ = billing::charge_request(
            &state.db,
            auth.user_id,
            model,
            prompt_tokens as u32,
            completion_tokens as u32,
        )
        .await;
    }
}

fn upstream_url(provider: &str) -> String {
    match provider {
        "openai" => "https://api.openai.com/v1/chat/completions".to_owned(),
        "anthropic" => "https://api.anthropic.com/v1/messages".to_owned(),
        "groq" => "https://api.groq.com/openai/v1/chat/completions".to_owned(),
        other => format!("https://api.{}.com/v1/chat/completions", other),
    }
}

fn provider_api_key(provider: &str) -> Result<String> {
    let env_var = format!("{}_API_KEY", provider.to_uppercase());
    std::env::var(&env_var)
        .map_err(|_| GatewayError::Internal(anyhow::anyhow!("API key ausente: {}", env_var)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upstream_url_known() {
        assert_eq!(
            upstream_url("openai"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            upstream_url("anthropic"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            upstream_url("groq"),
            "https://api.groq.com/openai/v1/chat/completions"
        );
    }

    #[test]
    fn test_upstream_url_unknown() {
        assert_eq!(
            upstream_url("custom"),
            "https://api.custom.com/v1/chat/completions"
        );
    }

    #[test]
    fn test_provider_api_key_present() {
        std::env::set_var("TESTPROV_API_KEY", "secret");
        assert_eq!(provider_api_key("testprov").unwrap(), "secret");
    }

    #[test]
    fn test_provider_api_key_missing() {
        let key = "MISSINGPROV_API_KEY";
        // Garante que não existe
        std::env::remove_var(key);
        assert!(provider_api_key("missingprov").is_err());
    }
}
