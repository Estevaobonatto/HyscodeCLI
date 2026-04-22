//! Ferramenta: escrita de arquivo.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult, traits::tool::Tool};
use serde_json::{json, Value};

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }

    fn description(&self) -> &str {
        "Escreve conteúdo em um arquivo. Cria o arquivo se não existir. Sobrescreve se existir."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Caminho do arquivo a ser escrito"
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
        let path = args["path"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'path' obrigatório".to_owned(),
        })?;
        let content = args["content"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'content' obrigatório".to_owned(),
        })?;

        // Cria diretórios pai se necessário
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await.map_err(ToolError::Io)?;
        }

        tokio::fs::write(path, content).await.map_err(ToolError::Io)?;

        Ok(ToolResult::success("", format!("Arquivo '{path}' escrito com sucesso.")))
    }
}
