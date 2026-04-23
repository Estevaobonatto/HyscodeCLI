//! Middleware de autenticação via Bearer token (`hsk_...`).
//!
//! Valida o token contra o banco de dados (hash SHA-256),
//! verifica se a key está ativa e não expirada,
//! e injeta o `AuthContext` no estado da requisição.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::GatewayError;

/// Contexto de autenticação injetado nas rotas.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    pub scopes: Vec<String>,
    pub rate_limit_rpm: i32,
}

/// Estado compartilhado para o middleware de auth.
#[derive(Clone)]
pub struct AuthState {
    pub db: PgPool,
}

/// Middleware axum que extrai e valida o Bearer token.
pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, GatewayError> {
    let token = extract_bearer_token(&req).ok_or(GatewayError::Unauthorized)?;

    // Apenas tokens com prefixo "hsk_" são aceitos
    if !token.starts_with("hsk_") {
        return Err(GatewayError::Unauthorized);
    }

    let key_hash = hash_token(&token);

    let row: Option<(Uuid, Uuid, Vec<String>, i32, bool, Option<chrono::DateTime<Utc>>)> =
        sqlx::query_as(
            r#"
            SELECT ak.user_id, ak.id, ak.scopes, ak.rate_limit_rpm, ak.is_active, ak.expires_at
            FROM api_keys ak
            WHERE ak.key_hash = $1
            "#,
        )
        .bind(&key_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    let (user_id, api_key_id, scopes, rate_limit_rpm, is_active, expires_at) =
        row.ok_or(GatewayError::Unauthorized)?;

    if !is_active {
        return Err(GatewayError::Forbidden);
    }

    if let Some(exp) = expires_at {
        if exp < Utc::now() {
            return Err(GatewayError::Forbidden);
        }
    }

    // Atualiza last_used_at de forma assíncrona (fire-and-forget)
    let db = state.db.clone();
    tokio::spawn(async move {
        let _ = sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
            .bind(api_key_id)
            .execute(&db)
            .await;
    });

    req.extensions_mut().insert(AuthContext {
        user_id,
        api_key_id,
        scopes,
        rate_limit_rpm,
    });

    Ok(next.run(req).await)
}

fn extract_bearer_token(req: &Request) -> Option<String> {
    let header = req.headers().get(axum::http::header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    let token = value.strip_prefix("Bearer ")?;
    Some(token.to_owned())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}
