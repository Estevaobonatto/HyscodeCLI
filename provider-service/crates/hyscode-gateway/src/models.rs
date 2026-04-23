//! Modelos de dados da API (request/response DTOs).

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Chat Completions (compatível com OpenAI)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: UsageInfo,
}

#[derive(Debug, Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Serialize, Default)]
pub struct UsageInfo {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Models list
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub object: String,
    pub data: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
}

// ---------------------------------------------------------------------------
// Health check
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: &'static str,
}

// ---------------------------------------------------------------------------
// Auth
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: String,
    pub email: String,
    pub display_name: Option<String>,
    pub tier: String,
    pub role: String,
}

// ---------------------------------------------------------------------------
// API Keys
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub label: Option<String>,
    pub scopes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub key_prefix: String,
    pub label: Option<String>,
    pub scopes: Vec<String>,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyWithSecret {
    pub id: String,
    pub key: String,
    pub key_prefix: String,
    pub label: Option<String>,
    pub scopes: Vec<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Dashboard / Usage
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct UsageSummary {
    pub total_requests: i64,
    pub total_tokens: i64,
    pub total_cost_cents: i64,
    pub current_month_tokens: i64,
    pub monthly_limit: i64,
    pub period_start: chrono::NaiveDate,
    pub period_end: chrono::NaiveDate,
}

#[derive(Debug, Serialize)]
pub struct UsageByModel {
    pub model: String,
    pub requests: i64,
    pub tokens: i64,
}

#[derive(Debug, Serialize)]
pub struct AlertItem {
    pub id: String,
    pub alert_type: String,
    pub message: String,
    pub is_read: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

// ---------------------------------------------------------------------------
// Billing
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct BillingRecordResponse {
    pub id: String,
    pub period_start: chrono::NaiveDate,
    pub period_end: chrono::NaiveDate,
    pub total_tokens: i64,
    pub total_requests: i32,
    pub total_cost_cents: i64,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct PlanResponse {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub monthly_limit_tokens: i64,
    pub monthly_price_cents: i32,
    pub features: Vec<String>,
}

// ---------------------------------------------------------------------------
// Admin
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreatePricingRequest {
    pub model_alias: String,
    pub provider: String,
    pub input_price_per_1k: i64,
    pub output_price_per_1k: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePricingRequest {
    pub input_price_per_1k: Option<i64>,
    pub output_price_per_1k: Option<i64>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct PricingResponse {
    pub id: String,
    pub model_alias: String,
    pub provider: String,
    pub input_price_per_1k: i64,
    pub output_price_per_1k: i64,
    pub currency: String,
    pub is_active: bool,
}

#[derive(Debug, Serialize)]
pub struct ProviderHealthItem {
    pub provider: String,
    pub checked_at: chrono::DateTime<chrono::Utc>,
    pub latency_ms: Option<i32>,
    pub status: String,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AdminMetrics {
    pub total_users: i64,
    pub active_users_7d: i64,
    pub total_requests_7d: i64,
    pub total_tokens_7d: i64,
    pub revenue_cents_7d: i64,
    pub top_models: Vec<UsageByModel>,
}
