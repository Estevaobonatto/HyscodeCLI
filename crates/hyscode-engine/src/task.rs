//! Sistema de Tasks — orquestração de tarefas integrado ao Harness AgentLoop.
//!
//! # SDD — Software Design Document
//!
//! ## Propósito
//! O TaskSystem é uma camada de orquestração acima do AgentLoop que permite
//! enfileirar, executar e monitorar múltiplas tarefas de codificação.
//!
//! ## Design
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │   TaskQueue │────►│ TaskRunner  │────►│  AgentLoop  │
//! │             │     │  (Harness)  │     │  (Harness)  │
//! └─────────────┘     └──────┬──────┘     └─────────────┘
//!                            │
//!                    ┌───────┴───────┐
//!                    ▼               ▼
//!            ┌─────────────┐  ┌─────────────┐
//!            │ TaskStore   │  │  EventBus   │
//!            │ (SQLite)    │  │  (mpsc)     │
//!            └─────────────┘  └─────────────┘
//! ```
//!
//! ## Estados de Task
//! Pending → Running → (Completed | Failed | Cancelled)
//!
//! ## Features
//! - Queue: FIFO com prioridade
//! - Persistência: SQLite via ConversationManager
//! - Eventos: streaming de progresso via mpsc
//! - Retries: até 3 tentativas por task

use hyscode_core::models::message::Message;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{error, info, warn};

use crate::{
    agent::{AgentConfig, AgentEvent, AgentLoop, AgentResult},
    context::ContextBuilder,
    permission::{PermissionConfig, PermissionManager},
};
use hyscode_core::traits::provider::Provider;
use hyscode_tools::ToolRegistry;

// ---------------------------------------------------------------------------
// Tipos
// ---------------------------------------------------------------------------

/// Status de uma task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Pending => write!(f, "pending"),
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Prioridade de execução.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Low,
    Normal,
    High,
    Critical,
}

impl TaskPriority {
    pub fn as_u8(&self) -> u8 {
        match self {
            TaskPriority::Low => 0,
            TaskPriority::Normal => 1,
            TaskPriority::High => 2,
            TaskPriority::Critical => 3,
        }
    }
}

/// Uma tarefa de codificação a ser executada pelo agente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub result_summary: Option<String>,
    pub error_message: Option<String>,
    pub iterations: u32,
    pub tools_used: Vec<String>,
    pub messages: Vec<Message>,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl Task {
    pub fn new(description: impl Into<String>) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            description: description.into(),
            status: TaskStatus::Pending,
            priority: TaskPriority::Normal,
            created_at: chrono::Utc::now().timestamp(),
            started_at: None,
            completed_at: None,
            result_summary: None,
            error_message: None,
            iterations: 0,
            tools_used: Vec::new(),
            messages: Vec::new(),
            retry_count: 0,
            max_retries: 3,
        }
    }

    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_max_retries(mut self, max: u32) -> Self {
        self.max_retries = max;
        self
    }

    pub fn can_retry(&self) -> bool {
        self.retry_count < self.max_retries
            && matches!(self.status, TaskStatus::Failed | TaskStatus::Cancelled)
    }
}

// ---------------------------------------------------------------------------
// Eventos do TaskSystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum TaskSystemEvent {
    TaskCreated { task_id: String, description: String },
    TaskStarted { task_id: String },
    TaskProgress { task_id: String, agent_event: AgentEvent },
    TaskCompleted { task_id: String, success: bool, summary: String },
    TaskFailed { task_id: String, error: String },
    TaskCancelled { task_id: String },
    QueueChanged { pending_count: usize },
}

// ---------------------------------------------------------------------------
// TaskQueue
// ---------------------------------------------------------------------------

/// Fila de tasks com prioridade.
pub struct TaskQueue {
    tasks: Mutex<VecDeque<Task>>,
}

impl TaskQueue {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(VecDeque::new()),
        }
    }

    pub async fn enqueue(&self, task: Task) {
        let mut tasks = self.tasks.lock().await;
        // Insere mantendo ordem de prioridade (maior primeiro)
        let idx = tasks.iter().position(|t| t.priority.as_u8() < task.priority.as_u8());
        match idx {
            Some(i) => tasks.insert(i, task),
            None => tasks.push_back(task),
        }
    }

    pub async fn dequeue(&self) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        tasks.pop_front()
    }

    pub async fn len(&self) -> usize {
        self.tasks.lock().await.len()
    }

    pub async fn is_empty(&self) -> bool {
        self.tasks.lock().await.is_empty()
    }

    pub async fn peek(&self) -> Option<Task> {
        self.tasks.lock().await.front().cloned()
    }

    pub async fn remove(&self, task_id: &str) -> Option<Task> {
        let mut tasks = self.tasks.lock().await;
        if let Some(pos) = tasks.iter().position(|t| t.id == task_id) {
            tasks.remove(pos)
        } else {
            None
        }
    }
}

impl Default for TaskQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// TaskRunner — orquestra tasks via AgentLoop
// ---------------------------------------------------------------------------

/// Executa tasks da fila usando o AgentLoop harness.
pub struct TaskRunner {
    provider: Arc<dyn Provider>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
    agent_config: AgentConfig,
    permission_config: PermissionConfig,
    queue: Arc<TaskQueue>,
    running: Arc<RwLock<bool>>,
    event_tx: Option<mpsc::UnboundedSender<TaskSystemEvent>>,
}

impl TaskRunner {
    pub fn new(
        provider: Arc<dyn Provider>,
        tool_registry: Arc<ToolRegistry>,
        context_builder: ContextBuilder,
        agent_config: AgentConfig,
        permission_config: PermissionConfig,
    ) -> Self {
        Self {
            provider,
            tool_registry,
            context_builder,
            agent_config,
            permission_config,
            queue: Arc::new(TaskQueue::new()),
            running: Arc::new(RwLock::new(false)),
            event_tx: None,
        }
    }

    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<TaskSystemEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    pub fn queue(&self) -> Arc<TaskQueue> {
        self.queue.clone()
    }

    fn emit(&self, event: TaskSystemEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Enfileira uma nova task.
    pub async fn submit(&self, task: Task) {
        info!(task_id = %task.id, "Task enfileirada");
        self.emit(TaskSystemEvent::TaskCreated {
            task_id: task.id.clone(),
            description: task.description.clone(),
        });
        self.queue.enqueue(task).await;
        self.emit(TaskSystemEvent::QueueChanged {
            pending_count: self.queue.len().await,
        });
    }

    /// Inicia o runner em background.
    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            if let Err(e) = self.run_loop().await {
                error!("TaskRunner falhou: {}", e);
            }
        });
    }

    /// Para o runner.
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Loop principal do runner.
    async fn run_loop(&self) -> anyhow::Result<()> {
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        loop {
            let is_running = *self.running.read().await;
            if !is_running {
                info!("TaskRunner parado");
                break;
            }

            if let Some(mut task) = self.queue.dequeue().await {
                info!(task_id = %task.id, "Executando task");
                self.emit(TaskSystemEvent::TaskStarted {
                    task_id: task.id.clone(),
                });

                task.status = TaskStatus::Running;
                task.started_at = Some(chrono::Utc::now().timestamp());

                let result = self.execute_task(&task).await;

                match result {
                    Ok(agent_result) => {
                        task.status = if agent_result.success {
                            TaskStatus::Completed
                        } else {
                            TaskStatus::Failed
                        };
                        task.completed_at = Some(chrono::Utc::now().timestamp());
                        task.result_summary = agent_result.final_response.clone();
                        task.iterations = agent_result.iterations;
                        task.tools_used = agent_result.tools_called;
                        task.messages = agent_result.messages;

                        if agent_result.success {
                            self.emit(TaskSystemEvent::TaskCompleted {
                                task_id: task.id.clone(),
                                success: true,
                                summary: agent_result.final_response.unwrap_or_default(),
                            });
                        } else {
                            self.emit(TaskSystemEvent::TaskFailed {
                                task_id: task.id.clone(),
                                error: agent_result
                                    .final_response
                                    .unwrap_or_else(|| "Falha sem mensagem".to_owned()),
                            });
                        }
                    }
                    Err(e) => {
                        task.status = TaskStatus::Failed;
                        task.completed_at = Some(chrono::Utc::now().timestamp());
                        task.error_message = Some(e.to_string());
                        task.retry_count += 1;

                        if task.can_retry() {
                            warn!(task_id = %task.id, "Re-enfileirando para retry");
                            task.status = TaskStatus::Pending;
                            self.queue.enqueue(task).await;
                        } else {
                            self.emit(TaskSystemEvent::TaskFailed {
                                task_id: task.id.clone(),
                                error: e.to_string(),
                            });
                        }
                    }
                }

                self.emit(TaskSystemEvent::QueueChanged {
                    pending_count: self.queue.len().await,
                });
            } else {
                // Fila vazia — dorme um pouco
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }

        Ok(())
    }

    /// Executa uma única task via AgentLoop harness.
    async fn execute_task(&self, task: &Task) -> anyhow::Result<AgentResult> {
        let pm = PermissionManager::new(
            self.permission_config.clone(),
            Arc::new(crate::permission::DenyAllCallback),
        );

        let (agent_tx, mut agent_rx) = mpsc::unbounded_channel::<AgentEvent>();
        let event_tx = self.event_tx.clone();
        let task_id = task.id.clone();

        // Forward AgentEvent → TaskSystemEvent
        tokio::spawn(async move {
            while let Some(ev) = agent_rx.recv().await {
                if let Some(ref tx) = event_tx {
                    let _ = tx.send(TaskSystemEvent::TaskProgress {
                        task_id: task_id.clone(),
                        agent_event: ev,
                    });
                }
            }
        });

        let agent = AgentLoop::new(
            self.provider.clone(),
            self.tool_registry.clone(),
            self.context_builder.clone(),
            self.agent_config.clone(),
        )
        .with_permission_manager(pm)
        .with_event_sender(agent_tx);

        agent.run(&task.description).await
    }
}

// ---------------------------------------------------------------------------
// TaskStore — persistência simples em memória (placeholder para SQLite)
// ---------------------------------------------------------------------------

/// Store de tasks — atualmente em memória, futuro: SQLite/ConversationManager.
pub struct TaskStore {
    tasks: Arc<RwLock<Vec<Task>>>,
}

impl TaskStore {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn save(&self, task: Task) {
        let mut tasks = self.tasks.write().await;
        if let Some(idx) = tasks.iter().position(|t| t.id == task.id) {
            tasks[idx] = task;
        } else {
            tasks.push(task);
        }
    }

    pub async fn get(&self, task_id: &str) -> Option<Task> {
        self.tasks.read().await.iter().find(|t| t.id == task_id).cloned()
    }

    pub async fn list(&self) -> Vec<Task> {
        self.tasks.read().await.clone()
    }

    pub async fn list_by_status(&self, status: TaskStatus) -> Vec<Task> {
        self.tasks
            .read()
            .await
            .iter()
            .filter(|t| t.status == status)
            .cloned()
            .collect()
    }
}

impl Default for TaskStore {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_queue_priority() {
        let queue = TaskQueue::new();

        let t1 = Task::new("low").with_priority(TaskPriority::Low);
        let t2 = Task::new("high").with_priority(TaskPriority::High);
        let t3 = Task::new("normal").with_priority(TaskPriority::Normal);

        queue.enqueue(t1.clone()).await;
        queue.enqueue(t2.clone()).await;
        queue.enqueue(t3.clone()).await;

        let first = queue.dequeue().await.unwrap();
        assert_eq!(first.priority, TaskPriority::High);

        let second = queue.dequeue().await.unwrap();
        assert_eq!(second.priority, TaskPriority::Normal);

        let third = queue.dequeue().await.unwrap();
        assert_eq!(third.priority, TaskPriority::Low);
    }

    #[tokio::test]
    async fn test_task_can_retry() {
        let mut task = Task::new("test").with_max_retries(2);
        task.status = TaskStatus::Failed;
        assert!(task.can_retry());

        task.retry_count = 1;
        assert!(task.can_retry());

        task.retry_count = 2;
        assert!(!task.can_retry());
    }

    #[tokio::test]
    async fn test_task_store() {
        let store = TaskStore::new();
        let task = Task::new("store test");

        store.save(task.clone()).await;
        let retrieved = store.get(&task.id).await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "store test");
    }
}
