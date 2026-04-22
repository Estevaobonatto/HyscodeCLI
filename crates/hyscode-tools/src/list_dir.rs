//! Ferramenta: listagem de diretório.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult, traits::tool::Tool};
use serde_json::{json, Value};

pub struct ListDirTool;

#[async_trait]
impl Tool for ListDirTool {
    fn name(&self) -> &str { "list_dir" }
    fn description(&self) -> &str { "Lista arquivos e subdiretórios de um diretório." }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Diretório a listar" }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let path = args["path"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'path' obrigatório".to_owned(),
        })?;

        let mut entries = tokio::fs::read_dir(path).await.map_err(ToolError::Io)?;
        let mut lines = Vec::new();

        while let Some(entry) = entries.next_entry().await.map_err(ToolError::Io)? {
            let meta = entry.metadata().await.map_err(ToolError::Io)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if meta.is_dir() {
                lines.push(format!("{name}/"));
            } else {
                lines.push(name);
            }
        }

        lines.sort();
        Ok(ToolResult::success("", lines.join("\n")))
    }
}
