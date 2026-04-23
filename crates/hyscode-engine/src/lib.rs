//! hyscode-engine — Orquestração de conversas e loop do agente

pub mod agent;
pub mod audit;
pub mod context;
pub mod conversation;
pub mod permission;
pub mod summarize;
pub mod task;
pub mod token;

pub use agent::{AgentConfig, AgentEvent, AgentLoop, AgentResult};
pub use audit::{audit_log_path, AuditEntry, AuditLog, AuditResult};
pub use context::ContextBuilder;
pub use conversation::ConversationManager;
pub use permission::{PermissionCallback, PermissionConfig, PermissionManager};
pub use summarize::maybe_summarize;
pub use task::{Task, TaskPriority, TaskQueue, TaskRunner, TaskStatus, TaskStore, TaskSystemEvent};
pub use token::TokenEstimator;
