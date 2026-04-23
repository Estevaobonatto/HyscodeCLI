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
    TaskCreated {
        task_id: String,
        description: String,
    },
    TaskStarted {
        task_id: String,
    },
    TaskProgress {
        task_id: String,
        agent_event: AgentEvent,
    },
    TaskCompleted {
        task_id: String,
        success: bool,
        summary: String,
    },
    TaskFailed {
        task_id: String,
        error: String,
    },
    TaskCancelled {
        task_id: String,
    },
    QueueChanged {
        pending_count: usize,
    },
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
        let idx = tasks
            .iter()
            .position(|t| t.priority.as_u8() < task.priority.as_u8());
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
    task_store: Option<Arc<TaskStore>>,
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
            task_store: None,
        }
    }

    pub fn with_task_store(mut self, store: Arc<TaskStore>) -> Self {
        self.task_store = Some(store);
        self
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

                if let Some(ref store) = self.task_store {
                    let _ = store.save(&task).await;
                }

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

                        if let Some(ref store) = self.task_store {
                            let _ = store.save(&task).await;
                        }

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

                        if let Some(ref store) = self.task_store {
                            let _ = store.save(&task).await;
                        }

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
// TaskStore — persistência SQLite
// ---------------------------------------------------------------------------

/// Store de tasks persistido em SQLite.
pub struct TaskStore {
    pool: sqlx::SqlitePool,
}

impl TaskStore {
    /// Abre (ou cria) o banco de dados em `~/.local/share/hyscode/tasks.db`.
    pub async fn new() -> anyhow::Result<Self> {
        let db_path = dirs::data_local_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("hyscode")
            .join("tasks.db");

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
        let pool = sqlx::SqlitePool::connect(&url).await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id            TEXT    PRIMARY KEY,
                description   TEXT    NOT NULL,
                status        TEXT    NOT NULL DEFAULT 'pending',
                priority      TEXT    NOT NULL DEFAULT 'normal',
                created_at    INTEGER NOT NULL,
                started_at    INTEGER,
                completed_at  INTEGER,
                result_summary TEXT,
                error_message  TEXT,
                iterations     INTEGER NOT NULL DEFAULT 0,
                tools_used     TEXT    NOT NULL DEFAULT '[]',
                messages       TEXT    NOT NULL DEFAULT '[]',
                retry_count    INTEGER NOT NULL DEFAULT 0,
                max_retries    INTEGER NOT NULL DEFAULT 3
            )
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn save(&self, task: &Task) -> anyhow::Result<()> {
        let tools_json = serde_json::to_string(&task.tools_used)?;
        let messages_json = serde_json::to_string(&task.messages)?;
        let status = task.status.to_string();
        let priority = serde_json::to_string(&task.priority)?
            .trim_matches('"')
            .to_owned();

        sqlx::query(
            r#"
            INSERT INTO tasks
                (id, description, status, priority, created_at, started_at, completed_at,
                 result_summary, error_message, iterations, tools_used, messages, retry_count, max_retries)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                status = excluded.status,
                started_at = excluded.started_at,
                completed_at = excluded.completed_at,
                result_summary = excluded.result_summary,
                error_message = excluded.error_message,
                iterations = excluded.iterations,
                tools_used = excluded.tools_used,
                messages = excluded.messages,
                retry_count = excluded.retry_count
            "#,
        )
        .bind(&task.id)
        .bind(&task.description)
        .bind(&status)
        .bind(&priority)
        .bind(task.created_at)
        .bind(task.started_at)
        .bind(task.completed_at)
        .bind(&task.result_summary)
        .bind(&task.error_message)
        .bind(task.iterations as i64)
        .bind(&tools_json)
        .bind(&messages_json)
        .bind(task.retry_count as i64)
        .bind(task.max_retries as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    #[allow(clippy::type_complexity)]
    pub async fn get(&self, task_id: &str) -> anyhow::Result<Option<Task>> {
        let row: Option<(
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            i64,
            String,
            String,
            i64,
            i64,
        )> = sqlx::query_as(
            r#"SELECT id, description, status, priority, created_at, started_at, completed_at,
                          result_summary, error_message, iterations, tools_used, messages,
                          retry_count, max_retries
                   FROM tasks WHERE id = ?"#,
        )
        .bind(task_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(Self::row_to_task))
    }

    #[allow(clippy::type_complexity)]
    pub async fn list(&self) -> anyhow::Result<Vec<Task>> {
        let rows: Vec<(
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            i64,
            String,
            String,
            i64,
            i64,
        )> = sqlx::query_as(
            r#"SELECT id, description, status, priority, created_at, started_at, completed_at,
                          result_summary, error_message, iterations, tools_used, messages,
                          retry_count, max_retries
                   FROM tasks ORDER BY created_at DESC"#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Self::row_to_task).collect())
    }

    #[allow(clippy::type_complexity)]
    pub async fn list_by_status(&self, status: TaskStatus) -> anyhow::Result<Vec<Task>> {
        let status_str = status.to_string();
        let rows: Vec<(
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            i64,
            String,
            String,
            i64,
            i64,
        )> = sqlx::query_as(
            r#"SELECT id, description, status, priority, created_at, started_at, completed_at,
                          result_summary, error_message, iterations, tools_used, messages,
                          retry_count, max_retries
                   FROM tasks WHERE status = ? ORDER BY created_at DESC"#,
        )
        .bind(&status_str)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(Self::row_to_task).collect())
    }

    #[allow(clippy::type_complexity)]
    fn row_to_task(
        row: (
            String,
            String,
            String,
            String,
            i64,
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            i64,
            String,
            String,
            i64,
            i64,
        ),
    ) -> Task {
        let (
            id,
            description,
            status_str,
            priority_str,
            created_at,
            started_at,
            completed_at,
            result_summary,
            error_message,
            iterations,
            tools_used_json,
            messages_json,
            retry_count,
            max_retries,
        ) = row;

        let status = match status_str.as_str() {
            "running" => TaskStatus::Running,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "cancelled" => TaskStatus::Cancelled,
            _ => TaskStatus::Pending,
        };

        let priority = match priority_str.as_str() {
            "low" => TaskPriority::Low,
            "high" => TaskPriority::High,
            "critical" => TaskPriority::Critical,
            _ => TaskPriority::Normal,
        };

        let tools_used: Vec<String> = serde_json::from_str(&tools_used_json).unwrap_or_default();
        let messages: Vec<Message> = serde_json::from_str(&messages_json).unwrap_or_default();

        Task {
            id,
            description,
            status,
            priority,
            created_at,
            started_at,
            completed_at,
            result_summary,
            error_message,
            iterations: iterations as u32,
            tools_used,
            messages,
            retry_count: retry_count as u32,
            max_retries: max_retries as u32,
        }
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
        let store = TaskStore::new().await.unwrap();
        let task = Task::new("store test");

        store.save(&task).await.unwrap();
        let retrieved = store.get(&task.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().description, "store test");
    }
}
