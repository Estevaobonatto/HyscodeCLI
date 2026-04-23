//! Registro de auditoria — persiste eventos de ferramentas e ações do agente em JSONL.
//!
//! Arquivo: `~/.local/share/hyscode/audit.jsonl` (Linux/macOS)
//!          `%APPDATA%\hyscode\audit.jsonl` (Windows)

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing::warn;

/// Uma entrada no log de auditoria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Timestamp ISO 8601
    pub timestamp: String,
    /// Nome da ferramenta ou ação (ex: `write_file`, `execute_command`)
    pub action: String,
    /// Argumentos da ação (JSON serializável)
    pub args: serde_json::Value,
    /// Resultado da operação (sucesso ou mensagem de erro)
    pub result: AuditResult,
    /// ID da conversa/sessão
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuditResult {
    /// Operação bem-sucedida
    Success,
    /// Operação falhou
    Failure { reason: String },
    /// Operação negada por política de permissões
    Denied { reason: String },
}

/// Gerencia o arquivo de auditoria em disco.
#[derive(Debug, Clone)]
pub struct AuditLog {
    path: PathBuf,
}

impl AuditLog {
    /// Cria um `AuditLog` apontando para o caminho padrão da plataforma.
    pub fn new() -> Self {
        let path = audit_log_path();
        Self { path }
    }

    /// Cria um `AuditLog` com caminho customizado (útil em testes).
    pub fn with_path(path: PathBuf) -> Self {
        Self { path }
    }

    /// Registra uma entrada no log de auditoria.
    ///
    /// A operação é assíncrona e não bloqueia o caller — falhas de I/O geram um
    /// `warn!` mas não propagam erros para não interromper o fluxo principal.
    pub async fn log(&self, entry: AuditEntry) {
        if let Err(e) = self.write_entry(&entry).await {
            warn!("Falha ao escrever no log de auditoria: {}", e);
        }
    }

    /// Registra uma ação de ferramenta bem-sucedida.
    pub async fn log_success(
        &self,
        action: &str,
        args: serde_json::Value,
        session_id: Option<String>,
    ) {
        self.log(AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            action: action.to_owned(),
            args,
            result: AuditResult::Success,
            session_id,
        })
        .await;
    }

    /// Registra uma ação de ferramenta que falhou.
    pub async fn log_failure(
        &self,
        action: &str,
        args: serde_json::Value,
        reason: impl Into<String>,
        session_id: Option<String>,
    ) {
        self.log(AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            action: action.to_owned(),
            args,
            result: AuditResult::Failure {
                reason: reason.into(),
            },
            session_id,
        })
        .await;
    }

    /// Registra uma ação que foi negada por permissão.
    pub async fn log_denied(
        &self,
        action: &str,
        args: serde_json::Value,
        reason: impl Into<String>,
        session_id: Option<String>,
    ) {
        self.log(AuditEntry {
            timestamp: Utc::now().to_rfc3339(),
            action: action.to_owned(),
            args,
            result: AuditResult::Denied {
                reason: reason.into(),
            },
            session_id,
        })
        .await;
    }

    async fn write_entry(&self, entry: &AuditEntry) -> anyhow::Result<()> {
        // Garante que o diretório existe
        if let Some(parent) = self.path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut line = serde_json::to_string(entry)?;
        line.push('\n');

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        Ok(())
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Retorna o caminho padrão do arquivo de auditoria para a plataforma atual.
pub fn audit_log_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));

    #[cfg(not(target_os = "windows"))]
    let base = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| PathBuf::from("."));

    base.join("hyscode").join("audit.jsonl")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_audit_log_creates_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("audit.jsonl");
        let log = AuditLog::with_path(path.clone());

        log.log_success("write_file", serde_json::json!({"path": "foo.rs"}), None)
            .await;

        assert!(path.exists());
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("write_file"));
        assert!(content.contains("success"));
    }

    #[tokio::test]
    async fn test_audit_log_appends() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("audit.jsonl");
        let log = AuditLog::with_path(path.clone());

        log.log_success("tool_a", serde_json::json!({}), None).await;
        log.log_failure("tool_b", serde_json::json!({}), "timeout", None)
            .await;

        let content = tokio::fs::read_to_string(&path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
    }
}
