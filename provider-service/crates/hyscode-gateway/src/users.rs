//! Auth de usuários — registro e login com JWT.

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{extract::State, Json};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{GatewayError, Result},
    models::{AuthResponse, LoginRequest, RegisterRequest, UserInfo},
    upstream::AppState,
};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    role: String,
}

/// POST /auth/register
pub async fn register_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>> {
    if req.email.is_empty() || req.password.len() < 8 {
        return Err(GatewayError::BadRequest(
            "Email inválido ou senha muito curta (mín 8)".to_owned(),
        ));
    }

    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
        .bind(&req.email)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| GatewayError::Internal(e.into()))?;

    if exists.is_some() {
        return Err(GatewayError::Conflict("Email já cadastrado".to_owned()));
    }

    let password_hash = hash_password(&req.password)?;
    let user_id = Uuid::new_v4();

    sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, display_name, tier, role)
        VALUES ($1, $2, $3, $4, 'free', 'user')
        "#,
    )
    .bind(user_id)
    .bind(&req.email)
    .bind(&password_hash)
    .bind(&req.display_name)
    .execute(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    // Cria quota inicial
    sqlx::query(
        r#"
        INSERT INTO usage_quotas (user_id, monthly_limit)
        VALUES ($1, 500000)
        ON CONFLICT (user_id) DO NOTHING
        "#,
    )
    .bind(user_id)
    .execute(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let token = generate_jwt(user_id, "user", &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user_id.to_string(),
            email: req.email,
            display_name: req.display_name,
            tier: "free".to_owned(),
            role: "user".to_owned(),
        },
    }))
}

/// POST /auth/login
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>> {
    let row: Option<(Uuid, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT id, password_hash, display_name, tier, role FROM users WHERE email = $1 AND is_active = TRUE"
    )
    .bind(&req.email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| GatewayError::Internal(e.into()))?;

    let (user_id, password_hash, display_name, tier, role) =
        row.ok_or_else(|| GatewayError::Unauthorized)?;

    verify_password(&req.password, &password_hash)?;

    let token = generate_jwt(user_id, &role, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user_id.to_string(),
            email: req.email,
            display_name,
            tier,
            role,
        },
    }))
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| GatewayError::Internal(anyhow::anyhow!("Hash falhou: {}", e)))?;
    Ok(hash.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| GatewayError::Internal(anyhow::anyhow!("Parse hash falhou: {}", e)))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| GatewayError::Unauthorized)?;
    Ok(())
}

fn generate_jwt(user_id: Uuid, role: &str, secret: &str) -> Result<String> {
    let exp = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        exp,
        role: role.to_owned(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| GatewayError::Internal(anyhow::anyhow!("JWT encode falhou: {}", e)))
}

/// Valida JWT e retorna (user_id, role).
pub fn validate_jwt(token: &str, secret: &str) -> Result<(Uuid, String)> {
    let token = token.strip_prefix("Bearer ").unwrap_or(token);
    let decoded = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| GatewayError::Unauthorized)?;

    let user_id = Uuid::parse_str(&decoded.claims.sub)
        .map_err(|e| GatewayError::Internal(anyhow::anyhow!("UUID inválido: {}", e)))?;

    Ok((user_id, decoded.claims.role))
}
