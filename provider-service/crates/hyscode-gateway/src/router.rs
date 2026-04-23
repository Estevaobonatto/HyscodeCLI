//! Router axum — define todas as rotas do gateway.

use std::sync::Arc;

use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use tower_http::{
    cors::CorsLayer,
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use crate::{
    auth::{auth_middleware, AuthState},
    config::Config,
    upstream::{
        chat_completions_handler, health_handler, list_models_handler, AppState,
    },
};

/// Constrói o router axum com todos os middlewares e rotas.
pub fn build_router(
    db: sqlx::PgPool,
    redis: redis::aio::ConnectionManager,
    config: Config,
) -> Router {
    let app_state = Arc::new(AppState::new(db.clone(), redis, config));
    let auth_state = Arc::new(AuthState { db });

    // Rotas públicas (sem autenticação)
    let public = Router::new()
        .route("/health", get(health_handler));

    // Rotas protegidas por Bearer token
    let protected = Router::new()
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/models", get(list_models_handler))
        .layer(middleware::from_fn_with_state(auth_state, auth_middleware));

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(app_state)
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(120)))
        .layer(CorsLayer::permissive())
}
