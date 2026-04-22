//! hyscode-engine — Orquestração de conversas e loop do agente

pub mod conversation;
pub mod context;
pub mod agent;
pub mod token;
pub mod permission;
pub mod task;

pub use conversation::ConversationManager;
pub use context::ContextBuilder;
pub use agent::{AgentLoop, AgentResult, AgentEvent, AgentConfig};
pub use token::TokenEstimator;
pub use permission::{PermissionManager, PermissionConfig, PermissionCallback};
pub use task::{Task, TaskQueue, TaskRunner, TaskStatus, TaskPriority, TaskStore, TaskSystemEvent};
