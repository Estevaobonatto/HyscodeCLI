//! Dashboard de uso para usuários autenticados.

use std::sync::Arc;

use axum::{extract::State, Extension, Json};
use chrono::{Datelike, NaiveDate, Utc};

use crate::{
    auth::AuthContext,
    error::{GatewayError, Result},
    models::{UsageByModel, UsageSummary},
    upstream::AppState,
};

/// GET /dashboard/usage
pub async fn usage_summary_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<UsageSummary>> {
    let period_start =
        NaiveDate::from_ymd_opt(Utc::now().year(), Utc::now().month(), 1).expect("data válida");
    let period_end = last_day_of_month(Utc::now().year(), Utc::now().month());

    let (total_requests, total_tokens, total_cost_cents): (Option<i64>, Option<i64>, Option<i64>) =
        sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(total_requests),0), COALESCE(SUM(total_tokens),0), COALESCE(SUM(total_cost_cents),0)
            FROM billing_records
            WHERE user_id = $1
            "#,
        )
        .bind(auth.user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    let (current_month_tokens, monthly_limit): (Option<i64>, Option<i64>) =
        sqlx::query_as("SELECT monthly_tokens, monthly_limit FROM usage_quotas WHERE user_id = $1")
            .bind(auth.user_id)
            .fetch_one(&state.db)
            .await
            .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(UsageSummary {
        total_requests: total_requests.unwrap_or(0),
        total_tokens: total_tokens.unwrap_or(0),
        total_cost_cents: total_cost_cents.unwrap_or(0),
        current_month_tokens: current_month_tokens.unwrap_or(0),
        monthly_limit: monthly_limit.unwrap_or(0),
        period_start,
        period_end,
    }))
}

/// GET /dashboard/usage/by-model
pub async fn usage_by_model_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<UsageByModel>>> {
    let rows: Vec<(String, i64, i64)> = sqlx::query_as(
        r#"
        SELECT model, COUNT(*) as requests, COALESCE(SUM(total_tokens),0) as tokens
        FROM requests_log
        WHERE user_id = $1 AND requested_at > NOW() - INTERVAL '30 days'
        GROUP BY model
        ORDER BY tokens DESC
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(
        rows.into_iter()
            .map(|(model, requests, tokens)| UsageByModel {
                model,
                requests,
                tokens,
            })
            .collect(),
    ))
}

fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("data válida")
        .pred_opt()
        .expect("data válida")
}
