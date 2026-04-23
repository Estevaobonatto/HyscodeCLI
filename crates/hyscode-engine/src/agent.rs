//! AgentLoop — Harness de execução autônoma de tarefas.
//!
//! # SDD — Software Design Document
//!
//! ## Propósito
//! O AgentLoop é o *harness* central que orquestra a execução de tarefas de
//! codificação de forma autônoma. Ele coordena Provedor LLM, Ferramentas,
//! Contexto e Permissões sem conhecer detalhes de implementação de nenhum
//! componente (arquitetura hexagonal).
//!
//! ## Harness Pattern
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │   Provider   │◄────│  AgentLoop   │────►│    Tools     │
//! │   (LLM)      │     │  (Harness)   │     │  (Registry)  │
//! └──────────────┘     └──────┬───────┘     └──────────────┘
//!                             │
//!                    ┌────────┴────────┐
//!                    ▼                 ▼
//!            ┌─────────────┐   ┌─────────────┐
//!            │  Permission │   │   Context   │
//!            │   Manager   │   │   Builder   │
//!            └─────────────┘   └─────────────┘
//! ```
//!
//! ## State Machine
//! ```text
//! Idle ──► Thinking ──► ToolCalls ──► Executing ──► Thinking ──► ...
//!                                              │
//!                                              ▼
//!                                          Done ──► Idle
//! ```
//!
//! ## Loop Invariant
//! 1. Cada iteração envia mensagens acumuladas + system prompt + tool schemas.
//! 2. Provider retorna ou texto (final) ou tool_calls (continua).
//! 3. Tool results são convertidos em mensagens `Message::Tool` e reapendadas.
//! 4. Max iterations evita loops infinitos.
//! 5. PermissionManager é gatekeeper de toda execução de tool.
//!
//! ## Event System
//! O harness emite `AgentEvent` para observabilidade externa (UI, logs,
//! métricas). Consumidores se inscrevem via `mpsc::channel`.
//!
//! ## Error Recovery
//! - Tool falha → erro é reportado ao modelo como ToolResult::is_error
//! - Provider falha → retry com backoff (configurável)
//! - Permissão negada → modelo recebe mensagem de erro
//! - Timeout → loop aborta com erro parcial

use hyscode_core::{
    error::{ProviderError, ToolError},
    models::{
        message::{Message, MessageContent},
        request::ChatRequest,
        response::ChatResponse,
        tool::{ToolCall, ToolResult},
    },
    traits::provider::Provider,
};
use hyscode_tools::ToolRegistry;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{
    audit::AuditLog,
    context::ContextBuilder,
    permission::{PermissionConfig, PermissionManager},
    summarize::maybe_summarize,
    token::TokenEstimator,
};

// ---------------------------------------------------------------------------
// Configuração
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub auto_approve: bool,
    pub audit_only: bool,
    pub confirm_writes: bool,
    pub confirm_commands: bool,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 15,
            auto_approve: false,
            audit_only: false,
            confirm_writes: true,
            confirm_commands: true,
            model: "gpt-4o".to_owned(),
            temperature: 0.2,
            max_tokens: 4096,
        }
    }
}

// ---------------------------------------------------------------------------
// Eventos do Harness
// ---------------------------------------------------------------------------

/// Evento emitido pelo AgentLoop para observadores externos.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    LoopStarted {
        task: String,
    },
    IterationStarted {
        iteration: u32,
        max: u32,
    },
    ProviderCalled {
        model: String,
        message_count: usize,
    },
    ProviderResponded {
        content_preview: Option<String>,
        tool_calls_count: usize,
    },
    ToolExecuting {
        name: String,
        args: Value,
    },
    ToolExecuted {
        name: String,
        success: bool,
        preview: String,
    },
    PermissionRequested {
        tool: String,
        args: Value,
    },
    PermissionGranted {
        tool: String,
    },
    PermissionDenied {
        tool: String,
        reason: String,
    },
    LoopFinished {
        success: bool,
        iterations: u32,
    },
    LoopError {
        error: String,
    },
    MessageAdded {
        role: String,
    },
}

// ---------------------------------------------------------------------------
// Resultado
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct AgentResult {
    pub success: bool,
    pub final_response: Option<String>,
    pub iterations: u32,
    pub tools_called: Vec<String>,
    pub messages: Vec<Message>,
}

// ---------------------------------------------------------------------------
// AgentLoop — Harness
// ---------------------------------------------------------------------------

pub struct AgentLoop {
    provider: Arc<dyn Provider>,
    tool_registry: Arc<ToolRegistry>,
    context_builder: ContextBuilder,
    config: AgentConfig,
    permission_manager: PermissionManager,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
    audit_log: AuditLog,
    token_estimator: TokenEstimator,
}

impl AgentLoop {
    pub fn new(
        provider: Arc<dyn Provider>,
        tool_registry: Arc<ToolRegistry>,
        context_builder: ContextBuilder,
        config: AgentConfig,
    ) -> Self {
        let perm_config = PermissionConfig {
            audit_only: config.audit_only,
            auto_approve_reads: !config.confirm_writes && !config.confirm_commands,
            auto_approve_all: config.auto_approve,
            confirm_timeout_secs: 60,
        };
        let permission_manager =
            PermissionManager::new(perm_config, Arc::new(crate::permission::DenyAllCallback));

        Self {
            provider,
            tool_registry,
            context_builder,
            config,
            permission_manager,
            event_tx: None,
            audit_log: AuditLog::new(),
            token_estimator: TokenEstimator::new(128_000),
        }
    }

    /// Injeta um sender de eventos para observabilidade.
    pub fn with_event_sender(mut self, tx: mpsc::UnboundedSender<AgentEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }

    /// Substitui o PermissionManager (útil para testes ou UI custom).
    pub fn with_permission_manager(mut self, pm: PermissionManager) -> Self {
        self.permission_manager = pm;
        self
    }

    /// Injeta um AuditLog customizado.
    pub fn with_audit_log(mut self, audit: AuditLog) -> Self {
        self.audit_log = audit;
        self
    }

    /// Injeta um TokenEstimator customizado.
    pub fn with_token_estimator(mut self, estimator: TokenEstimator) -> Self {
        self.token_estimator = estimator;
        self
    }

    fn emit(&self, event: AgentEvent) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(event);
        }
    }

    /// Executa o harness para completar a tarefa.
    ///
    /// SDD-LOOP-001: O loop é o coração do harness. Ele mantém invariantes
    /// de estado e garante que cada iteração tenha acesso ao histórico completo.
    pub async fn run(&self, task: &str) -> anyhow::Result<AgentResult> {
        info!(task = %task, "AgentLoop iniciado");
        self.emit(AgentEvent::LoopStarted {
            task: task.to_owned(),
        });

        // 1. Monta mensagens iniciais
        let system_prompt = self.context_builder.build_system_prompt().await?;
        let mut messages = vec![
            Message::System {
                content: system_prompt,
            },
            Message::User {
                content: MessageContent::Text(task.to_owned()),
            },
        ];

        let mut tools_called: Vec<String> = Vec::new();
        let mut iterations = 0u32;

        // 2. Loop principal do harness
        while iterations < self.config.max_iterations {
            iterations += 1;
            debug!(iteration = iterations, "nova iteração");
            self.emit(AgentEvent::IterationStarted {
                iteration: iterations,
                max: self.config.max_iterations,
            });

            // 2a. Auto-sumarização se contexto excede limite
            if let Ok(Some(condensed)) = maybe_summarize(
                &messages,
                self.provider.clone(),
                &self.config.model,
                &self.token_estimator,
            )
            .await
            {
                messages = condensed;
            }

            // 2b. Prepara request com tool schemas
            let tool_defs = self.tool_registry.to_definitions();
            let request = ChatRequest::new(self.config.model.clone(), messages.clone())
                .with_tools(tool_defs)
                .with_temperature(self.config.temperature)
                .with_max_tokens(self.config.max_tokens);

            self.emit(AgentEvent::ProviderCalled {
                model: self.config.model.clone(),
                message_count: messages.len(),
            });

            // 2c. Chama provider
            let response = match self.call_provider(request).await {
                Ok(resp) => resp,
                Err(e) => {
                    error!(error = %e, "Provider falhou");
                    self.emit(AgentEvent::LoopError {
                        error: e.to_string(),
                    });
                    return Ok(AgentResult {
                        success: false,
                        final_response: Some(format!("Erro do provedor: {e}")),
                        iterations,
                        tools_called,
                        messages,
                    });
                }
            };

            let content_preview = response.content.clone();
            let has_tool_calls = response.tool_calls.is_some();
            self.emit(AgentEvent::ProviderResponded {
                content_preview: content_preview.clone(),
                tool_calls_count: response.tool_calls.as_ref().map(|v| v.len()).unwrap_or(0),
            });

            // 2d. Se modelo retornou texto → tarefa concluída
            if !has_tool_calls {
                info!(
                    iterations = iterations,
                    "AgentLoop concluído sem tool_calls"
                );
                self.emit(AgentEvent::LoopFinished {
                    success: true,
                    iterations,
                });
                messages.push(Message::Assistant {
                    content: content_preview.clone(),
                    tool_calls: None,
                    thinking: None,
                });
                return Ok(AgentResult {
                    success: true,
                    final_response: content_preview,
                    iterations,
                    tools_called,
                    messages,
                });
            }

            // 2e. Modelo pediu tool_calls → executa cada uma
            let tool_calls = response.tool_calls.unwrap_or_default();
            messages.push(Message::Assistant {
                content: content_preview,
                tool_calls: Some(tool_calls.clone()),
                thinking: None,
            });
            self.emit(AgentEvent::MessageAdded {
                role: "assistant".to_owned(),
            });

            let mut all_tool_results = Vec::new();
            for call in tool_calls {
                let tool_name = call.name.clone();
                tools_called.push(tool_name.clone());

                let result = self.execute_tool_call(&call).await;
                all_tool_results.push((call, result));
            }

            // 2f. Converte resultados em mensagens Tool
            for (call, result) in all_tool_results {
                let tool_result = match result {
                    Ok(tr) => tr,
                    Err(e) => ToolResult::error(&call.id, format!("Erro: {e}")),
                };

                messages.push(Message::Tool {
                    tool_call_id: call.id.clone(),
                    content: tool_result.content.clone(),
                    is_error: tool_result.is_error,
                });
                self.emit(AgentEvent::MessageAdded {
                    role: "tool".to_owned(),
                });
            }
        }

        // 3. Max iterations atingido
        warn!(iterations = iterations, "max iterations atingido");
        self.emit(AgentEvent::LoopFinished {
            success: false,
            iterations,
        });
        Ok(AgentResult {
            success: false,
            final_response: Some(
                "Não consegui completar a tarefa no número máximo de iterações.".to_owned(),
            ),
            iterations,
            tools_called,
            messages,
        })
    }

    /// Chama o provider com retry simples.
    async fn call_provider(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let max_retries = 3u32;
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.provider.chat(request.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    warn!(attempt = attempt, error = %e, "provider error, retrying");
                    last_error = Some(e);
                    if attempt < max_retries - 1 {
                        tokio::time::sleep(std::time::Duration::from_secs(2u64.pow(attempt))).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(ProviderError::Unavailable))
    }

    /// Executa uma única tool_call com permission check.
    async fn execute_tool_call(&self, call: &ToolCall) -> Result<ToolResult, ToolError> {
        let tool_name = &call.name;
        let args = call.parse_args().unwrap_or(Value::Null);

        self.emit(AgentEvent::ToolExecuting {
            name: tool_name.clone(),
            args: args.clone(),
        });

        let tool = self
            .tool_registry
            .get(tool_name)
            .ok_or_else(|| ToolError::NotFound(tool_name.clone()))?;

        // Permission check via harness
        let requires_confirmation = tool.requires_confirmation();
        let is_destructive = tool.is_destructive();

        self.emit(AgentEvent::PermissionRequested {
            tool: tool_name.clone(),
            args: args.clone(),
        });

        match self
            .permission_manager
            .check(tool_name, &args, requires_confirmation, is_destructive)
            .await
        {
            Ok(()) => {
                self.emit(AgentEvent::PermissionGranted {
                    tool: tool_name.clone(),
                });
            }
            Err(e) => {
                self.emit(AgentEvent::PermissionDenied {
                    tool: tool_name.clone(),
                    reason: e.to_string(),
                });
                self.audit_log
                    .log_denied(tool_name, args.clone(), e.to_string(), None)
                    .await;
                return Err(e);
            }
        }

        // Executa a tool
        let result = tool.execute(args.clone()).await;

        match &result {
            Ok(tr) => {
                let preview = if tr.content.len() > 200 {
                    format!("{}...", &tr.content[..200])
                } else {
                    tr.content.clone()
                };
                self.emit(AgentEvent::ToolExecuted {
                    name: tool_name.clone(),
                    success: !tr.is_error,
                    preview,
                });
                if tr.is_error {
                    self.audit_log
                        .log_failure(tool_name, args, &tr.content, None)
                        .await;
                } else {
                    self.audit_log.log_success(tool_name, args, None).await;
                }
            }
            Err(e) => {
                self.emit(AgentEvent::ToolExecuted {
                    name: tool_name.clone(),
                    success: false,
                    preview: e.to_string(),
                });
                self.audit_log
                    .log_failure(tool_name, args, e.to_string(), None)
                    .await;
            }
        }

        result
    }
}

// ---------------------------------------------------------------------------
// MockProvider — para testes do harness
// ---------------------------------------------------------------------------

#[cfg(test)]
use async_trait::async_trait;
#[cfg(test)]
use futures::stream::BoxStream;
#[cfg(test)]
use hyscode_core::models::{
    provider::{ModelInfo, ProviderCapabilities},
    response::{ChatChunk, Delta, FinishReason},
};

#[cfg(test)]
pub struct MockProvider {
    pub responses: std::sync::Mutex<Vec<ChatResponse>>,
    pub call_count: std::sync::Mutex<usize>,
}

#[cfg(test)]
#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_tools: true,
            supports_streaming: false,
            supports_system_prompt: true,
            supports_vision: false,
            supports_parallel_tool_calls: false,
            max_context_tokens: 128_000,
        }
    }

    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        let mut count = self.call_count.lock().unwrap();
        let responses = self.responses.lock().unwrap();
        let idx = *count % responses.len();
        *count += 1;
        Ok(responses[idx].clone())
    }

    async fn chat_stream(
        &self,
        _request: ChatRequest,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        let resp = self.chat(_request).await?;
        let chunk = ChatChunk {
            id: resp.id.clone(),
            delta: Delta {
                role: Some("assistant".to_owned()),
                content: resp.content.clone(),
                tool_call_delta: None,
            },
            finish_reason: Some(resp.finish_reason.clone()),
            usage: Some(resp.usage.clone()),
        };
        let stream = futures::stream::iter(vec![Ok(chunk)]);
        Ok(Box::pin(stream))
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(vec![])
    }

    async fn validate(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Testes do Harness
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use hyscode_core::models::usage::TokenUsage;

    fn mock_provider_with_responses(responses: Vec<ChatResponse>) -> Arc<MockProvider> {
        Arc::new(MockProvider {
            responses: std::sync::Mutex::new(responses),
            call_count: std::sync::Mutex::new(0),
        })
    }

    fn make_tool_response(content: &str) -> ChatResponse {
        ChatResponse {
            id: "resp1".to_owned(),
            model: "mock".to_owned(),
            content: Some(content.to_owned()),
            tool_calls: None,
            finish_reason: FinishReason::Stop,
            usage: TokenUsage::default(),
        }
    }

    fn make_tool_call_response(calls: Vec<ToolCall>) -> ChatResponse {
        ChatResponse {
            id: "resp2".to_owned(),
            model: "mock".to_owned(),
            content: None,
            tool_calls: Some(calls),
            finish_reason: FinishReason::ToolCalls,
            usage: TokenUsage::default(),
        }
    }

    fn setup_agent(provider: Arc<MockProvider>) -> AgentLoop {
        let tool_registry = Arc::new(ToolRegistry::with_defaults());
        let context = ContextBuilder::new(std::env::current_dir().unwrap());
        let config = AgentConfig {
            max_iterations: 5,
            auto_approve: true,
            ..AgentConfig::default()
        };
        let pm = PermissionManager::approve_all();

        AgentLoop::new(provider, tool_registry, context, config).with_permission_manager(pm)
    }

    #[tokio::test]
    async fn test_agent_completes_without_tools() {
        let provider = mock_provider_with_responses(vec![make_tool_response("Tarefa concluída.")]);
        let agent = setup_agent(provider.clone());
        let result = agent.run("teste simples").await.unwrap();

        assert!(result.success);
        assert_eq!(result.final_response, Some("Tarefa concluída.".to_owned()));
        assert_eq!(result.iterations, 1);
    }

    #[tokio::test]
    async fn test_agent_executes_tool_then_completes() {
        let provider = mock_provider_with_responses(vec![
            make_tool_call_response(vec![ToolCall {
                id: "call_1".to_owned(),
                name: "read_file".to_owned(),
                arguments: r#"{"path":"Cargo.toml"}"#.to_owned(),
            }]),
            make_tool_response("Li o arquivo."),
        ]);
        let agent = setup_agent(provider.clone());
        let result = agent.run("leia o Cargo.toml").await.unwrap();

        assert!(result.success);
        assert_eq!(result.final_response, Some("Li o arquivo.".to_owned()));
        assert_eq!(result.iterations, 2);
        assert!(result.tools_called.contains(&"read_file".to_owned()));
    }

    #[tokio::test]
    async fn test_agent_respects_max_iterations() {
        // Sempre retorna tool_call → loop infinito simulado
        let provider =
            mock_provider_with_responses(vec![make_tool_call_response(vec![ToolCall {
                id: "call_1".to_owned(),
                name: "read_file".to_owned(),
                arguments: r#"{"path":"foo"}"#.to_owned(),
            }])]);
        let agent = setup_agent(provider.clone());
        let result = agent.run("loop infinito").await.unwrap();

        assert!(!result.success);
        assert_eq!(result.iterations, 5); // max_iterations do config
    }

    #[tokio::test]
    async fn test_agent_permission_denied_reports_error() {
        // Mock: tool_call → permission denied → model recebe erro → responde texto
        let provider = mock_provider_with_responses(vec![
            make_tool_call_response(vec![ToolCall {
                id: "call_1".to_owned(),
                name: "write_file".to_owned(),
                arguments: r#"{"path":"x","content":"y"}"#.to_owned(),
            }]),
            make_tool_response("Não consegui escrever: permissão negada."),
        ]);

        let tool_registry = Arc::new(ToolRegistry::with_defaults());
        let context = ContextBuilder::new(std::env::current_dir().unwrap());
        let config = AgentConfig {
            max_iterations: 3,
            auto_approve: false,
            ..AgentConfig::default()
        };

        // PermissionManager que nega tudo
        let agent = AgentLoop::new(provider, tool_registry, context, config);
        let result = agent.run("tenta escrever").await.unwrap();

        // Iteração 1: tool_call → permission denied → tool result erro
        // Iteração 2: provider responde texto → loop termina
        assert_eq!(result.iterations, 2);
        assert!(result.tools_called.contains(&"write_file".to_owned()));
    }

    #[tokio::test]
    async fn test_event_stream() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let provider = mock_provider_with_responses(vec![make_tool_response("ok")]);
        let agent = setup_agent(provider).with_event_sender(tx);

        let _ = agent.run("test events").await.unwrap();

        let mut events = vec![];
        while let Ok(e) = rx.try_recv() {
            events.push(e);
        }

        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::LoopStarted { .. })),
            "deve ter LoopStarted"
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, AgentEvent::LoopFinished { .. })),
            "deve ter LoopFinished"
        );
    }
}
