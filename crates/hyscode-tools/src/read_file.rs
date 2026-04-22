//! Ferramenta: leitura de arquivo.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult, traits::tool::Tool};
use serde_json::{json, Value};

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }

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
        let path = args["path"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidArgs {
                tool: self.name().to_owned(),
                reason: "campo 'path' obrigatório".to_owned(),
            })?;

        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            ToolError::Io(e)
        })?;

        Ok(ToolResult::success("", content))
    }
}
