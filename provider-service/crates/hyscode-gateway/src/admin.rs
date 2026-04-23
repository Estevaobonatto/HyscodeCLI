//! Admin — gestão de provedores, preços e métricas de negócio.

use std::sync::Arc;

use axum::{extract::State, Json};
use chrono::{Duration, Utc};
use uuid::Uuid;

use crate::{
    error::{GatewayError, Result},
    models::{
        AdminMetrics, CreatePricingRequest, PricingResponse, ProviderHealthItem,
        UpdatePricingRequest, UsageByModel,
    },
    upstream::AppState,
};

/// GET /admin/pricing
pub async fn list_pricing_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PricingResponse>>> {
    let rows: Vec<(Uuid, String, String, i64, i64, String, bool)> = sqlx::query_as(
        "SELECT id, model_alias, provider, input_price_per_1k, output_price_per_1k, currency, is_active FROM pricing_per_model ORDER BY model_alias"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    model_alias,
                    provider,
                    input_price_per_1k,
                    output_price_per_1k,
                    currency,
                    is_active,
                )| PricingResponse {
                    id: id.to_string(),
                    model_alias,
                    provider,
                    input_price_per_1k,
                    output_price_per_1k,
                    currency,
                    is_active,
                },
            )
            .collect(),
    ))
}

/// POST /admin/pricing
pub async fn create_pricing_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreatePricingRequest>,
) -> Result<Json<PricingResponse>> {
    let id = Uuid::new_v4();
    sqlx::query(
        r#"
        INSERT INTO pricing_per_model (id, model_alias, provider, input_price_per_1k, output_price_per_1k)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(id)
    .bind(&req.model_alias)
    .bind(&req.provider)
    .bind(req.input_price_per_1k)
    .bind(req.output_price_per_1k)
    .execute(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(PricingResponse {
        id: id.to_string(),
        model_alias: req.model_alias,
        provider: req.provider,
        input_price_per_1k: req.input_price_per_1k,
        output_price_per_1k: req.output_price_per_1k,
        currency: "USD".to_owned(),
        is_active: true,
    }))
}

/// PUT /admin/pricing/:id
pub async fn update_pricing_handler(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Json(req): Json<UpdatePricingRequest>,
) -> Result<Json<serde_json::Value>> {
    let mut query = "UPDATE pricing_per_model SET updated_at = NOW()".to_owned();
    let mut binds: Vec<String> = vec![];

    if let Some(input) = req.input_price_per_1k {
        query.push_str(", input_price_per_1k = $2");
        binds.push(input.to_string());
    }
    if let Some(output) = req.output_price_per_1k {
        query.push_str(", output_price_per_1k = $3");
        binds.push(output.to_string());
    }
    if let Some(active) = req.is_active {
        query.push_str(", is_active = $4");
        binds.push(active.to_string());
    }

    query.push_str(" WHERE id = $1");

    let mut q = sqlx::query(&query).bind(id);
    for b in &binds {
        q = q.bind(b);
    }

    let result = q
        .execute(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::NotFound("Preço não encontrado".to_owned()));
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

/// GET /admin/provider-health
#[allow(clippy::type_complexity)]
pub async fn list_provider_health_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProviderHealthItem>>> {
    let rows: Vec<(
        String,
        chrono::DateTime<chrono::Utc>,
        Option<i32>,
        String,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT provider, checked_at, latency_ms, status, error_message
        FROM provider_health
        ORDER BY checked_at DESC
        LIMIT 100
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(provider, checked_at, latency_ms, status, error_message)| ProviderHealthItem {
                    provider,
                    checked_at,
                    latency_ms,
                    status,
                    error_message,
                },
            )
            .collect(),
    ))
}

/// POST /admin/provider-health/check
pub async fn check_providers_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ProviderHealthItem>>> {
    let mut results = vec![];

    for route in &state.config.model_routes {
        let url = upstream_url(&route.provider);
        let start = std::time::Instant::now();
        let status = match state.http.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => "healthy",
            Ok(_) => "degraded",
            Err(_) => "down",
        };
        let latency_ms = start.elapsed().as_millis() as i32;
        let error_message = if status == "down" {
            Some("Falha ao conectar".to_owned())
        } else {
            None
        };

        sqlx::query(
            "INSERT INTO provider_health (provider, latency_ms, status, error_message, endpoint_url) VALUES ($1, $2, $3, $4, $5)"
        )
        .bind(&route.provider)
        .bind(latency_ms)
        .bind(status)
        .bind(&error_message)
        .bind(&url)
        .execute(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

        results.push(ProviderHealthItem {
            provider: route.provider.clone(),
            checked_at: Utc::now(),
            latency_ms: Some(latency_ms),
            status: status.to_owned(),
            error_message,
        });
    }

    Ok(Json(results))
}

/// GET /admin/metrics
pub async fn admin_metrics_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AdminMetrics>> {
    let since = Utc::now() - Duration::days(7);

    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    let active_users_7d: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT user_id) FROM requests_log WHERE requested_at > $1",
    )
    .bind(since)
    .fetch_one(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let total_requests_7d: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM requests_log WHERE requested_at > $1")
            .bind(since)
            .fetch_one(&state.db)
            .await
            .map_err(|e| GatewayError::Internal(e.into()))?;

    let total_tokens_7d: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_tokens),0) FROM requests_log WHERE requested_at > $1",
    )
    .bind(since)
    .fetch_one(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let revenue_cents_7d: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(total_cost_cents),0) FROM billing_records WHERE updated_at > $1",
    )
    .bind(since)
    .fetch_one(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let top_models: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT model, COUNT(*) as requests, COALESCE(SUM(total_tokens),0) as tokens
        FROM requests_log
        WHERE requested_at > $1
        GROUP BY model
        ORDER BY tokens DESC
        LIMIT 5
        "#,
    )
    .bind(since)
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(AdminMetrics {
        total_users,
        active_users_7d,
        total_requests_7d,
        total_tokens_7d,
        revenue_cents_7d,
        top_models: top_models
            .into_iter()
            .map(|(model, requests, tokens)| UsageByModel {
                model,
                requests,
                tokens,
            })
            .collect(),
    }))
}

fn upstream_url(provider: &str) -> String {
    match provider {
        "openai" => "https://api.openai.com/v1/models".to_owned(),
        "anthropic" => "https://api.anthropic.com/v1/models".to_owned(),
        "groq" => "https://api.groq.com/openai/v1/models".to_owned(),
        other => format!("https://api.{}.com/v1/models", other),
    }
}
