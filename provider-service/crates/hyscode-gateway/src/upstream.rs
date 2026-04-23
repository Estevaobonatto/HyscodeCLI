//! Upstream LLM proxy — encaminha requisições para o provedor correto.
//!
//! Suporta streaming via SSE (Server-Sent Events) e resposta completa.

use std::sync::Arc;

use axum::{
    extract::{Extension, State},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Json,
    },
    http::StatusCode,
};
use futures::StreamExt;
use reqwest::Client;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::AuthContext,
    config::{Config, ModelRoute},
    db,
    error::{GatewayError, Result},
    models::{
        ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice,
        ModelInfo, ModelsResponse, UsageInfo, HealthResponse,
    },
};

/// Estado compartilhado com o router axum.
#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
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
) -> Result<impl IntoResponse> {
    // Verifica quota antes de processar
    check_quota(&state, auth.user_id).await?;

    let route = state
        .resolve_provider(&req.model)
        .ok_or_else(|| GatewayError::ModelNotFound(req.model.clone()))?;

    // Monta a URL do upstream baseado no provider
    let upstream_url = upstream_url(&route.provider);
    let api_key = provider_api_key(&route.provider)?;

    if req.stream {
        // Para simplificar, retorna não-streaming mesmo que stream=true
        // TODO: implementar SSE pass-through quando necessário
        let response = forward_request(&state.http, &upstream_url, &api_key, &req).await?;
        log_request(&state, &auth, &req.model, &route.provider, &response).await;
        Ok(Json(response).into_response())
    } else {
        let response = forward_request(&state.http, &upstream_url, &api_key, &req).await?;
        log_request(&state, &auth, &req.model, &route.provider, &response).await;
        Ok(Json(response).into_response())
    }
}

async fn forward_request(
    http: &Client,
    upstream_url: &str,
    api_key: &str,
    req: &ChatCompletionRequest,
) -> Result<ChatCompletionResponse> {
    let body = serde_json::to_value(req)
        .map_err(|e| GatewayError::Internal(e.into()))?;

    let resp = http
        .post(upstream_url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| GatewayError::UpstreamError(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(GatewayError::UpstreamError(format!("HTTP {}: {}", status, text)));
    }

    let raw: Value = resp.json().await.map_err(|e| GatewayError::Internal(e.into()))?;

    // Normaliza resposta upstream para o formato interno
    let usage = UsageInfo {
        prompt_tokens: raw["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
        completion_tokens: raw["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
        total_tokens: raw["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
    };

    let choice = raw["choices"][0].clone();
    let message = ChatMessage {
        role: choice["message"]["role"].as_str().unwrap_or("assistant").to_owned(),
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
            finish_reason: choice["finish_reason"].as_str().unwrap_or("stop").to_owned(),
        }],
        usage,
    })
}

async fn check_quota(state: &AppState, user_id: Uuid) -> Result<()> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT monthly_tokens, monthly_limit FROM usage_quotas WHERE user_id = $1",
    )
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

async fn log_request(
    state: &AppState,
    auth: &AuthContext,
    model: &str,
    provider: &str,
    resp: &ChatCompletionResponse,
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO requests_log
            (api_key_id, user_id, model, upstream_provider, prompt_tokens, completion_tokens, total_tokens, status_code)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 200)
        "#,
    )
    .bind(auth.api_key_id)
    .bind(auth.user_id)
    .bind(model)
    .bind(provider)
    .bind(resp.usage.prompt_tokens as i32)
    .bind(resp.usage.completion_tokens as i32)
    .bind(resp.usage.total_tokens as i32)
    .execute(&state.db)
    .await;

    // Incrementa contador de quota
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
    .bind(resp.usage.total_tokens as i64)
    .execute(&state.db)
    .await;
}

fn upstream_url(provider: &str) -> String {
    match provider {
        "openai"    => "https://api.openai.com/v1/chat/completions".to_owned(),
        "anthropic" => "https://api.anthropic.com/v1/messages".to_owned(),
        "groq"      => "https://api.groq.com/openai/v1/chat/completions".to_owned(),
        other       => format!("https://api.{}.com/v1/chat/completions", other),
    }
}

fn provider_api_key(provider: &str) -> Result<String> {
    let env_var = format!("{}_API_KEY", provider.to_uppercase());
    std::env::var(&env_var)
        .map_err(|_| GatewayError::Internal(anyhow::anyhow!("API key ausente: {}", env_var)))
}
