//! Ferramenta: leitura de arquivo.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult, traits::tool::Tool};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub struct ReadFileTool;

/// Resolve e valida que `target` está contido dentro de `base` (previne path traversal).
/// Compartilhado com write_file via inline copy — sem dep extra.
fn resolve_safe_path(base: &Path, target: &str) -> Result<PathBuf, ToolError> {
    let raw = Path::new(target);
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        base.join(raw)
    };
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

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        use std::path::Component;
        match comp {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out
}

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Lê o conteúdo completo de um arquivo. Use caminhos relativos ao diretório de trabalho."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Caminho do arquivo a ser lido"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path_str = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs {
                tool: self.name().to_owned(),
                reason: "campo 'path' obrigatório".to_owned(),
            })?;

        let cwd = std::env::current_dir().map_err(ToolError::Io)?;
        let safe_path = resolve_safe_path(&cwd, path_str)?;

        let content = tokio::fs::read_to_string(&safe_path)
            .await
            .map_err(ToolError::Io)?;

        Ok(ToolResult::success("", content))
    }
}
