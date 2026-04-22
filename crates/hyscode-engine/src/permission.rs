//! PermissionManager — policy engine para execução de ferramentas.
//!
//! SDD: O PermissionManager é o ponto central de controle de acesso do harness.
//! Ele decide se uma tool pode executar baseado em:
//!   - Configuração global (audit_only, auto_approve)
//!   - Natureza da tool (destructive vs read-only)
//!   - Callback interativo (para confirmação do usuário)
//!
//! Design principle: fail-closed. Sem permissão explícita = negação.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Callback para confirmação interativa de ferramentas destrutivas.
///
/// Implementado pela camada de UI (CLI/TUI) para exibir prompts ao usuário.
/// No modo não-interativo (CI/CD), retorna false sempre.
#[async_trait]
pub trait PermissionCallback: Send + Sync {
    /// Pergunta ao usuário se pode executar a ferramenta.
    /// `destructive` indica se a tool modifica dados.
    async fn confirm(&self, tool_name: &str, args: &Value, destructive: bool) -> bool;
}

/// Callback padrão: sempre nega (safe default para ambientes não-interativos).
pub struct DenyAllCallback;

#[async_trait]
impl PermissionCallback for DenyAllCallback {
    async fn confirm(&self, tool_name: &str, _args: &Value, _destructive: bool) -> bool {
        warn!(tool = %tool_name, "DenyAllCallback: negação automática");
        false
    }
}

/// Callback para testes: sempre aprova.
pub struct ApproveAllCallback;

#[async_trait]
impl PermissionCallback for ApproveAllCallback {
    async fn confirm(&self, tool_name: &str, _args: &Value, _destructive: bool) -> bool {
        debug!(tool = %tool_name, "ApproveAllCallback: aprovação automática");
        true
    }
}

/// Configuração de permissões do AgentLoop.
#[derive(Debug, Clone)]
pub struct PermissionConfig {
    /// Se true, nunca executa — só loga o que faria.
    pub audit_only: bool,
    /// Se true, aprova automaticamente ferramentas não-destrutivas.
    pub auto_approve_reads: bool,
    /// Se true, aprova automaticamente TUDO (uso em CI/testes apenas).
    pub auto_approve_all: bool,
    /// Timeout para esperar confirmação do usuário.
    pub confirm_timeout_secs: u64,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            audit_only: false,
            auto_approve_reads: true,
            auto_approve_all: false,
            confirm_timeout_secs: 60,
        }
    }
}

/// PermissionManager — policy engine do harness.
pub struct PermissionManager {
    config: PermissionConfig,
    callback: Arc<dyn PermissionCallback>,
}

impl PermissionManager {
    pub fn new(config: PermissionConfig, callback: Arc<dyn PermissionCallback>) -> Self {
        Self { config, callback }
    }

    /// Cria com config padrão e callback que nega tudo (safe default).
    pub fn default_deny() -> Self {
        Self::new(PermissionConfig::default(), Arc::new(DenyAllCallback))
    }

    /// Cria com callback que aprova tudo (apenas para testes).
    pub fn approve_all() -> Self {
        Self::new(
            PermissionConfig {
                auto_approve_all: true,
                ..PermissionConfig::default()
            },
            Arc::new(ApproveAllCallback),
        )
    }

    /// Verifica se uma tool pode executar.
    ///
    /// Returns:
    /// - `Ok(())` → pode executar
    /// - `Err(ToolError::PermissionDenied)` → negação (audit ou usuário negou)
    /// - `Err(ToolError::Cancelled)` → timeout ou cancelamento
    pub async fn check(
        &self,
        tool_name: &str,
        args: &Value,
        requires_confirmation: bool,
        is_destructive: bool,
    ) -> Result<(), ToolError> {
        if self.config.audit_only {
            info!(
                tool = %tool_name,
                args = %args,
                "[AUDIT-ONLY] Would execute tool"
            );
            return Err(ToolError::PermissionDenied(format!(
                "audit-only mode: would execute {tool_name} with args {args}"
            )));
        }

        if self.config.auto_approve_all {
            debug!(tool = %tool_name, "auto_approve_all ativo");
            return Ok(());
        }

        if !requires_confirmation && !is_destructive && self.config.auto_approve_reads {
            debug!(tool = %tool_name, "auto-approved (read-only)");
            return Ok(());
        }

        // Tool destrutiva ou que requer confirmação → pergunta ao callback
        let approved = tokio::time::timeout(
            std::time::Duration::from_secs(self.config.confirm_timeout_secs),
            self.callback.confirm(tool_name, args, is_destructive),
        )
        .await
        .map_err(|_| {
            warn!(tool = %tool_name, "timeout na confirmação");
            ToolError::Cancelled
        })?;

        if approved {
            info!(tool = %tool_name, "usuário aprovou execução");
            Ok(())
        } else {
            warn!(tool = %tool_name, "usuário negou execução");
            Err(ToolError::PermissionDenied(format!(
                "usuário negou execução de {tool_name}"
            )))
        }
    }

    /// Executa uma tool com permission check prévio.
    ///
    /// Harness pattern: o PermissionManager é o gatekeeper.
    /// A tool só executa se todas as policies passarem.
    pub async fn execute_with_check<T, F>(
        &self,
        tool_name: &str,
        args: &Value,
        requires_confirmation: bool,
        is_destructive: bool,
        exec_fn: F,
    ) -> Result<T, ToolError>
    where
        F: std::future::Future<Output = Result<T, ToolError>>,
    {
        self.check(tool_name, args, requires_confirmation, is_destructive)
            .await?;
        exec_fn.await
    }
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_audit_only_denies_all() {
        let pm = PermissionManager::new(
            PermissionConfig {
                audit_only: true,
                ..PermissionConfig::default()
            },
            Arc::new(ApproveAllCallback),
        );

        let result = pm.check("write_file", &Value::Null, false, true).await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_auto_approve_reads() {
        let pm = PermissionManager::default_deny();
        // read_file não é destrutiva e não requer confirmação
        let result = pm.check("read_file", &Value::Null, false, false).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_deny_destructive_without_auto_approve() {
        let pm = PermissionManager::default_deny();
        let result = pm.check("write_file", &Value::Null, true, true).await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }

    #[tokio::test]
    async fn test_approve_all_allows_destructive() {
        let pm = PermissionManager::approve_all();
        let result = pm.check("write_file", &Value::Null, true, true).await;
        assert!(result.is_ok());
    }
}
