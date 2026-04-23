//! Billing — cálculo de custo, alertas e quotas.

use std::sync::Arc;

use axum::{extract::State, Extension, Json};
use chrono::{Datelike, NaiveDate, Utc};
use uuid::Uuid;

use crate::{
    auth::AuthContext,
    error::{GatewayError, Result},
    models::{AlertItem, BillingRecordResponse, PlanResponse},
    upstream::AppState,
};

/// GET /billing/plans
pub async fn list_plans_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<PlanResponse>>> {
    let rows: Vec<(Uuid, String, String, i64, i32, Vec<String>)> = sqlx::query_as(
        "SELECT id, name, tier, monthly_limit_tokens, monthly_price_cents, features FROM subscription_plans WHERE is_active = TRUE ORDER BY monthly_price_cents"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, name, tier, monthly_limit_tokens, monthly_price_cents, features)| {
                    PlanResponse {
                        id: id.to_string(),
                        name,
                        tier,
                        monthly_limit_tokens,
                        monthly_price_cents,
                        features,
                    }
                },
            )
            .collect(),
    ))
}

/// GET /billing/records
pub async fn list_billing_records_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<BillingRecordResponse>>> {
    let rows: Vec<(Uuid, NaiveDate, NaiveDate, i64, i32, i64, String)> = sqlx::query_as(
        r#"
        SELECT id, period_start, period_end, total_tokens, total_requests, total_cost_cents, status
        FROM billing_records
        WHERE user_id = $1
        ORDER BY period_start DESC
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    period_start,
                    period_end,
                    total_tokens,
                    total_requests,
                    total_cost_cents,
                    status,
                )| BillingRecordResponse {
                    id: id.to_string(),
                    period_start,
                    period_end,
                    total_tokens,
                    total_requests,
                    total_cost_cents,
                    status,
                },
            )
            .collect(),
    ))
}

/// GET /billing/alerts
pub async fn list_alerts_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<AlertItem>>> {
    let rows: Vec<(Uuid, String, String, bool, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
        r#"
        SELECT id, alert_type, message, is_read, created_at
        FROM usage_alerts
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(id, alert_type, message, is_read, created_at)| AlertItem {
                id: id.to_string(),
                alert_type,
                message,
                is_read,
                created_at,
            })
            .collect(),
    ))
}

/// POST /billing/alerts/:id/read
pub async fn mark_alert_read_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let result =
        sqlx::query("UPDATE usage_alerts SET is_read = TRUE WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(auth.user_id)
            .execute(&state.db)
            .await
            .map_err(|e| GatewayError::Internal(e.into()))?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::NotFound("Alerta não encontrado".to_owned()));
    }

    Ok(Json(serde_json::json!({ "read": true })))
}

// ---------------------------------------------------------------------------
// Internals — chamado por upstream após cada request
// ---------------------------------------------------------------------------

/// Calcula custo baseado em pricing_per_model e grava billing record.
pub async fn charge_request(
    db: &sqlx::PgPool,
    user_id: Uuid,
    model: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
) -> Result<()> {
    let pricing: Option<(i64, i64)> = sqlx::query_as(
        "SELECT input_price_per_1k, output_price_per_1k FROM pricing_per_model WHERE model_alias = $1 AND is_active = TRUE"
    )
    .bind(model)
    .fetch_optional(db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let cost_cents = if let Some((input_price, output_price)) = pricing {
        let input_cost = (prompt_tokens as i64 * input_price) / 1_000_000; // price em centavos de centavo / 1k
        let output_cost = (completion_tokens as i64 * output_price) / 1_000_000;
        (input_cost + output_cost) / 100 // converte para centavos
    } else {
        0
    };

    let period_start =
        NaiveDate::from_ymd_opt(Utc::now().year(), Utc::now().month(), 1).expect("data válida");
    let period_end = last_day_of_month(Utc::now().year(), Utc::now().month());

    sqlx::query(
        r#"
        INSERT INTO billing_records (user_id, period_start, period_end, total_tokens, total_requests, total_cost_cents, status)
        VALUES ($1, $2, $3, $4, 1, $5, 'open')
        ON CONFLICT (user_id, period_start, period_end) DO UPDATE
        SET total_tokens = billing_records.total_tokens + $4,
            total_requests = billing_records.total_requests + 1,
            total_cost_cents = billing_records.total_cost_cents + $5,
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(period_start)
    .bind(period_end)
    .bind((prompt_tokens + completion_tokens) as i64)
    .bind(cost_cents)
    .execute(db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    // Verifica alertas
    check_alerts(db, user_id).await?;

    Ok(())
}

async fn check_alerts(db: &sqlx::PgPool, user_id: Uuid) -> Result<()> {
    let row: Option<(i64, i64)> =
        sqlx::query_as("SELECT monthly_tokens, monthly_limit FROM usage_quotas WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(db)
            .await
            .map_err(|e| GatewayError::Internal(e.into()))?;

    if let Some((used, limit)) = row {
        if limit > 0 {
            let pct = (used * 100) / limit;
            if pct >= 100 {
                create_alert_if_not_exists(
                    db,
                    user_id,
                    "quota_100",
                    "Quota mensal de tokens atingida (100%).",
                )
                .await?;
            } else if pct >= 80 {
                create_alert_if_not_exists(
                    db,
                    user_id,
                    "quota_80",
                    "Quota mensal de tokens atingiu 80%.",
                )
                .await?;
            }
        }
    }

    Ok(())
}

async fn create_alert_if_not_exists(
    db: &sqlx::PgPool,
    user_id: Uuid,
    alert_type: &str,
    message: &str,
) -> Result<()> {
    let exists: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM usage_alerts WHERE user_id = $1 AND alert_type = $2 AND created_at > NOW() - INTERVAL '1 day'"
    )
    .bind(user_id)
    .bind(alert_type)
    .fetch_optional(db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    if exists.is_none() {
        sqlx::query("INSERT INTO usage_alerts (user_id, alert_type, message) VALUES ($1, $2, $3)")
            .bind(user_id)
            .bind(alert_type)
            .bind(message)
            .execute(db)
            .await
            .map_err(|e| GatewayError::Internal(e.into()))?;
    }

    Ok(())
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("data válida")
        .pred_opt()
        .expect("data válida")
}
