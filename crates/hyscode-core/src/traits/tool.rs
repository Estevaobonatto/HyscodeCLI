//! Trait de ferramenta do agente.

use crate::{error::ToolError, models::tool::ToolResult};
use async_trait::async_trait;
use serde_json::Value;

/// Porta para ferramentas executáveis pelo agente.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Nome único da ferramenta (ex: "read_file").
    fn name(&self) -> &str;

    /// Descrição legível da ferramenta para o modelo.
    fn description(&self) -> &str;

    /// JSON Schema dos parâmetros de entrada.
    fn schema(&self) -> Value;

    /// Se `true`, o usuário deve confirmar antes da execução.
    fn requires_confirmation(&self) -> bool {
        false
    }

    /// Se `true`, a operação modifica ou destrói dados.
    fn is_destructive(&self) -> bool {
        false
    }

    /// Executa a ferramenta com os argumentos fornecidos.
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}
