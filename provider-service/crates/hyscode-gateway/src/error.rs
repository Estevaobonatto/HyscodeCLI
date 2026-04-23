//! Erros do gateway.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("Não autenticado")]
    Unauthorized,

    #[error("Acesso negado")]
    Forbidden,

    #[error("Quota de tokens excedida")]
    QuotaExceeded,

    #[error("Modelo não encontrado: {0}")]
    ModelNotFound(String),

    #[error("Provedor upstream indisponível: {0}")]
    UpstreamError(String),

    #[error("Requisição inválida: {0}")]
    BadRequest(String),

    #[error("Erro interno: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            GatewayError::Unauthorized    => (StatusCode::UNAUTHORIZED,    "unauthorized",      self.to_string()),
            GatewayError::Forbidden       => (StatusCode::FORBIDDEN,       "forbidden",         self.to_string()),
            GatewayError::QuotaExceeded   => (StatusCode::TOO_MANY_REQUESTS, "quota_exceeded",  self.to_string()),
            GatewayError::ModelNotFound(_) => (StatusCode::NOT_FOUND,      "model_not_found",   self.to_string()),
            GatewayError::UpstreamError(_) => (StatusCode::BAD_GATEWAY,    "upstream_error",    self.to_string()),
            GatewayError::BadRequest(_)   => (StatusCode::BAD_REQUEST,     "bad_request",       self.to_string()),
            GatewayError::Internal(e)     => {
                tracing::error!("Erro interno: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", "Erro interno do servidor".to_owned())
            }
        };

        (
            status,
            Json(json!({
                "error": {
                    "code": code,
                    "message": message
                }
            })),
        )
            .into_response()
    }
}

pub type Result<T> = std::result::Result<T, GatewayError>;
