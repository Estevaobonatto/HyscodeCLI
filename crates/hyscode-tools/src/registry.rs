//! Registro de ferramentas disponíveis ao agente.

use hyscode_core::traits::tool::Tool;
use std::{collections::HashMap, sync::Arc};

/// Registro central de ferramentas.
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Cria um registry com todas as ferramentas nativas.
    pub fn with_defaults() -> Self {
        use crate::*;
        let mut registry = Self::new();
        registry.register(Arc::new(read_file::ReadFileTool));
        registry.register(Arc::new(write_file::WriteFileTool));
        registry.register(Arc::new(list_dir::ListDirTool));
        registry.register(Arc::new(search_code::SearchCodeTool));
        registry.register(Arc::new(execute_command::ExecuteCommandTool));
        registry.register(Arc::new(git_diff::GitDiffTool));
        registry
    }

    /// Registra uma ferramenta.
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_owned(), tool);
    }

    /// Obtém uma ferramenta pelo nome.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// Lista todas as ferramentas como definições JSON Schema para o modelo.
    pub fn to_definitions(&self) -> Vec<hyscode_core::models::tool::ToolDefinition> {
        self.tools
            .values()
            .map(|t| hyscode_core::models::tool::ToolDefinition {
                name: t.name().to_owned(),
                description: t.description().to_owned(),
                parameters: t.schema(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}
