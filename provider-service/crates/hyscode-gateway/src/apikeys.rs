//! CRUD de API keys para usuários autenticados.

use std::sync::Arc;

use axum::{extract::State, Extension, Json};
use rand::Rng;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    auth::AuthContext,
    error::{GatewayError, Result},
    models::{ApiKeyResponse, ApiKeyWithSecret, CreateApiKeyRequest},
    upstream::AppState,
};

/// GET /apikeys
#[allow(clippy::type_complexity)]
pub async fn list_apikeys_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ApiKeyResponse>>> {
    let rows: Vec<(
        Uuid,
        String,
        Option<String>,
        Vec<String>,
        bool,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
            SELECT id, key_prefix, label, scopes, is_active, created_at
            FROM api_keys
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let items = rows
        .into_iter()
        .map(
            |(id, key_prefix, label, scopes, is_active, created_at)| ApiKeyResponse {
                id: id.to_string(),
                key_prefix,
                label,
                scopes,
                is_active,
                created_at,
            },
        )
        .collect();

    Ok(Json(items))
}

/// POST /apikeys
pub async fn create_apikey_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyWithSecret>> {
    let key = generate_api_key();
    let key_hash = hash_key(&key);
    let key_prefix = key.chars().take(12).collect::<String>();
    let api_key_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO api_keys (id, user_id, key_hash, key_prefix, label, scopes)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(api_key_id)
    .bind(auth.user_id)
    .bind(&key_hash)
    .bind(&key_prefix)
    .bind(&req.label)
    .bind(&req.scopes)
    .execute(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    Ok(Json(ApiKeyWithSecret {
        id: api_key_id.to_string(),
        key,
        key_prefix,
        label: req.label,
        scopes: req.scopes,
        created_at: chrono::Utc::now(),
    }))
}

/// DELETE /apikeys/:id
pub async fn delete_apikey_handler(
    Extension(auth): Extension<AuthContext>,
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<Json<serde_json::Value>> {
    let result = sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    if result.rows_affected() == 0 {
        return Err(GatewayError::NotFound("API key não encontrada".to_owned()));
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

fn generate_api_key() -> String {
    let mut rng = rand::thread_rng();
    let suffix: String = (0..32)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect();
    format!("hsk_{}", suffix)
}

fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hex::encode(hasher.finalize())
}
