//! Router axum — define todas as rotas do gateway.

use std::sync::Arc;

use axum::{
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};

use crate::{
    admin, apikeys,
    auth::{admin_middleware, auth_middleware, jwt_auth_middleware, AuthState},
    billing,
    config::Config,
    dashboard,
    upstream::{chat_completions_handler, health_handler, list_models_handler, AppState},
    users,
};

/// Constrói o router axum com todos os middlewares e rotas.
pub fn build_router(
    db: sqlx::PgPool,
    redis: redis::aio::ConnectionManager,
    config: Config,
) -> Router {
    let app_state = Arc::new(AppState::new(db.clone(), redis.clone(), config));
    let auth_state = Arc::new(AuthState {
        db: db.clone(),
        redis,
    });

    // Rotas públicas
    let public = Router::new()
        .route("/health", get(health_handler))
        .route("/auth/register", post(users::register_handler))
        .route("/auth/login", post(users::login_handler));

    // Rotas protegidas por API Key (OpenAI-compatible)
    let api_key_protected = Router::new()
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/models", get(list_models_handler))
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            auth_middleware,
        ));

    // Rotas protegidas por JWT (web dashboard / CLI user)
    let jwt_protected = Router::new()
        .route(
            "/apikeys",
            get(apikeys::list_apikeys_handler).post(apikeys::create_apikey_handler),
        )
        .route("/apikeys/:id", delete(apikeys::delete_apikey_handler))
        .route("/dashboard/usage", get(dashboard::usage_summary_handler))
        .route(
            "/dashboard/usage/by-model",
            get(dashboard::usage_by_model_handler),
        )
        .route("/billing/plans", get(billing::list_plans_handler))
        .route(
            "/billing/records",
            get(billing::list_billing_records_handler),
        )
        .route("/billing/alerts", get(billing::list_alerts_handler))
        .route(
            "/billing/alerts/:id/read",
            post(billing::mark_alert_read_handler),
        )
        .layer(middleware::from_fn_with_state(
            auth_state.clone(),
            jwt_auth_middleware,
        ));

    // Rotas admin (JWT + admin role)
    let admin_routes = Router::new()
        .route(
            "/admin/pricing",
            get(admin::list_pricing_handler).post(admin::create_pricing_handler),
        )
        .route("/admin/pricing/:id", put(admin::update_pricing_handler))
        .route(
            "/admin/provider-health",
            get(admin::list_provider_health_handler),
        )
        .route(
            "/admin/provider-health/check",
            post(admin::check_providers_handler),
        )
        .route("/admin/metrics", get(admin::admin_metrics_handler))
        .layer(middleware::from_fn(admin_middleware))
        .layer(middleware::from_fn_with_state(
            auth_state,
            jwt_auth_middleware,
        ));

    Router::new()
        .merge(public)
        .merge(api_key_protected)
        .merge(jwt_protected)
        .merge(admin_routes)
        .with_state(app_state)
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(120)))
        .layer(CorsLayer::permissive())
}
