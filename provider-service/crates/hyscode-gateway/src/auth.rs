//! Middleware de autenticação via Bearer token (`hsk_...`).
//!
//! Valida token contra DB (hash SHA-256),
//! verifica key ativa/não expirada,
//! aplica rate limiting via Redis,
//! injeta `AuthContext` na requisição.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::GatewayError;
use crate::users::validate_jwt;

/// Contexto de autenticação injetado nas rotas.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub user_id: Uuid,
    pub api_key_id: Uuid,
    #[allow(dead_code)]
    pub scopes: Vec<String>,
    #[allow(dead_code)]
    pub rate_limit_rpm: i32,
    pub role: String,
}

/// Estado compartilhado para middleware de auth.
#[derive(Clone)]
pub struct AuthState {
    pub db: PgPool,
    pub redis: redis::aio::ConnectionManager,
}

/// Middleware axum que extrai e valida Bearer token.
pub async fn auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, GatewayError> {
    let token = extract_bearer_token(&req).ok_or(GatewayError::Unauthorized)?;

    if !token.starts_with("hsk_") {
        return Err(GatewayError::Unauthorized);
    }

    let key_hash = hash_token(&token);

    #[allow(clippy::type_complexity)]
    let row: Option<(Uuid, Uuid, Vec<String>, i32, bool, Option<chrono::DateTime<Utc>>, String)> =
        sqlx::query_as(
            r#"
            SELECT ak.user_id, ak.id, ak.scopes, ak.rate_limit_rpm, ak.is_active, ak.expires_at, u.role
            FROM api_keys ak
            JOIN users u ON u.id = ak.user_id
            WHERE ak.key_hash = $1
            "#,
        )
        .bind(&key_hash)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    let (user_id, api_key_id, scopes, rate_limit_rpm, is_active, expires_at, role) =
        row.ok_or(GatewayError::Unauthorized)?;

    if !is_active {
        return Err(GatewayError::Forbidden);
    }

    if let Some(exp) = expires_at {
        if exp < Utc::now() {
            return Err(GatewayError::Forbidden);
        }
    }

    // Rate limiting via Redis (sliding window simples)
    let rate_key = format!("rate_limit:{}", api_key_id);
    let current: i64 = state.redis.clone().get(&rate_key).await.unwrap_or(0);

    if current >= rate_limit_rpm as i64 {
        return Err(GatewayError::QuotaExceeded);
    }

    let mut redis_conn = state.redis.clone();
    let _: Result<(), _> = redis_conn.incr(&rate_key, 1).await;
    let _: Result<(), _> = redis_conn.expire(&rate_key, 60).await;

    // Atualiza last_used_at fire-and-forget
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
        role,
    });

    Ok(next.run(req).await)
}

/// Middleware para exigir role admin.
pub async fn admin_middleware(
    req: Request,
    next: Next,
) -> std::result::Result<Response, GatewayError> {
    let auth = req.extensions().get::<AuthContext>().cloned();
    match auth {
        Some(ctx) if ctx.role == "admin" => Ok(next.run(req).await),
        _ => Err(GatewayError::Forbidden),
    }
}

/// Middleware axum que valida JWT e injeta AuthContext.
pub async fn jwt_auth_middleware(
    State(state): State<Arc<AuthState>>,
    mut req: Request,
    next: Next,
) -> std::result::Result<Response, GatewayError> {
    let token = extract_bearer_token(&req).ok_or(GatewayError::Unauthorized)?;

    let (user_id, role) = validate_jwt(&token, &std::env::var("JWT_SECRET").unwrap_or_default())?;

    let row: Option<(Uuid, Vec<String>, i32)> = sqlx::query_as(
        "SELECT id, scopes, rate_limit_rpm FROM api_keys WHERE user_id = $1 AND is_active = TRUE LIMIT 1"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let (api_key_id, scopes, rate_limit_rpm) = match row {
        Some((id, scopes, rpm)) => (id, scopes, rpm),
        None => (Uuid::nil(), vec![], 60),
    };

    req.extensions_mut().insert(AuthContext {
        user_id,
        api_key_id,
        scopes,
        rate_limit_rpm,
        role,
    });

    Ok(next.run(req).await)
}

fn extract_bearer_token<B>(req: &Request<B>) -> Option<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_token_consistent() {
        let h1 = hash_token("hsk_abc123");
        let h2 = hash_token("hsk_abc123");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }

    #[test]
    fn test_extract_bearer_token_ok() {
        let req = Request::builder()
            .header("Authorization", "Bearer hsk_test")
            .body(())
            .unwrap();
        let token = extract_bearer_token(&req);
        assert_eq!(token, Some("hsk_test".to_owned()));
    }

    #[test]
    fn test_extract_bearer_token_missing() {
        let req = Request::builder().body(()).unwrap();
        assert_eq!(extract_bearer_token(&req), None);
    }

    #[test]
    fn test_extract_bearer_token_bad_prefix() {
        let req = Request::builder()
            .header("Authorization", "Basic foo")
            .body(())
            .unwrap();
        assert_eq!(extract_bearer_token(&req), None);
    }
}
