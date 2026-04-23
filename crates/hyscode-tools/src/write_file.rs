//! Ferramenta: escrita de arquivo.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult, traits::tool::Tool};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct WriteFileTool;

/// Resolve e valida que `target` está contido dentro de `base` (previne path traversal).
fn resolve_safe_path(base: &Path, target: &str) -> Result<PathBuf, ToolError> {
    let raw = Path::new(target);
    // Se for absoluto, usar diretamente; se relativo, prefixar com base.
    let joined = if raw.is_absolute() { raw.to_path_buf() } else { base.join(raw) };

    // Normalizar sem precisar que o arquivo exista (canonicalize exige existência).
    let normalized = normalize_path(&joined);
    let base_norm = normalize_path(base);

    if !normalized.starts_with(&base_norm) {
        return Err(ToolError::PermissionDenied(format!(
            "path '{}' está fora do diretório de trabalho '{}'.",
            target,
            base_norm.display()
        )));
    }
    Ok(normalized)
}

/// Normaliza um path sem precisar que exista (resolve `.` e `..` lexicamente).
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::ParentDir => { out.pop(); }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }

    fn description(&self) -> &str {
        "Escreve conteúdo em um arquivo dentro do diretório de trabalho. Cria o arquivo se não existir. Sobrescreve se existir."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Caminho do arquivo a ser escrito (relativo ao cwd)"
                },
                "content": {
                    "type": "string",
                    "description": "Conteúdo a ser escrito no arquivo"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn requires_confirmation(&self) -> bool { true }
    fn is_destructive(&self) -> bool { true }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path_str = args["path"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'path' obrigatório".to_owned(),
        })?;
        let content = args["content"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'content' obrigatório".to_owned(),
        })?;

        let cwd = std::env::current_dir().map_err(ToolError::Io)?;
        let safe_path = resolve_safe_path(&cwd, path_str)?;

        // Cria diretórios pai se necessário
        if let Some(parent) = safe_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(ToolError::Io)?;
        }

        // Backup do arquivo existente para suporte a rollback
        let backup_path = if safe_path.exists() {
            match backup_file(&safe_path).await {
                Ok(bp) => Some(bp),
                Err(e) => {
                    tracing::warn!("Falha ao criar backup de '{}': {}", safe_path.display(), e);
                    None
                }
            }
        } else {
            None
        };

        tokio::fs::write(&safe_path, content).await.map_err(ToolError::Io)?;

        // Registra no undo log
        if let Some(ref bp) = backup_path {
            let _ = append_undo_entry(&safe_path, bp).await;
        }

        let msg = if let Some(bp) = backup_path {
            format!(
                "Arquivo '{}' escrito com sucesso. Backup em '{}'.",
                safe_path.display(),
                bp.display()
            )
        } else {
            format!("Arquivo '{}' criado com sucesso.", safe_path.display())
        };

        Ok(ToolResult::success("", msg))
    }
}

/// Cria uma cópia de segurança do arquivo antes de sobrescrever.
async fn backup_file(original: &Path) -> std::io::Result<PathBuf> {
    let backup_dir = backup_dir_path();
    tokio::fs::create_dir_all(&backup_dir).await?;

    // Usa hash do path + timestamp para nome único
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let file_name = original
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_owned());

    let backup_name = format!("{}.{}.bak", file_name, ts);
    let backup_path = backup_dir.join(backup_name);
    tokio::fs::copy(original, &backup_path).await?;
    Ok(backup_path)
}

/// Caminho do diretório de backups.
fn backup_dir_path() -> std::path::PathBuf {
    dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hyscode")
        .join("backups")
}

/// Adiciona uma entrada no undo log (JSONL).
async fn append_undo_entry(original: &Path, backup: &Path) -> std::io::Result<()> {
    let undo_log = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".local").join("share")))
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("hyscode")
        .join("undo.jsonl");

    let entry = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "original": original.to_string_lossy(),
        "backup": backup.to_string_lossy(),
    });

    let mut line = serde_json::to_string(&entry).unwrap_or_default();
    line.push('\n');

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&undo_log)
        .await?;

    use tokio::io::AsyncWriteExt;
    file.write_all(line.as_bytes()).await?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_safe_path_relative() {
        let base = PathBuf::from("/home/user/project");
        let result = resolve_safe_path(&base, "src/main.rs");
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_safe_path_traversal_blocked() {
        let base = PathBuf::from("/home/user/project");
        let result = resolve_safe_path(&base, "../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_safe_path_absolute_outside_blocked() {
        let base = PathBuf::from("/home/user/project");
        let result = resolve_safe_path(&base, "/etc/shadow");
        assert!(result.is_err());
    }
}
