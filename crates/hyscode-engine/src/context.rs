//! Construção do contexto para requisições ao modelo.

use std::path::PathBuf;

/// Monta o contexto completo a ser enviado ao modelo.
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    pub working_dir: PathBuf,
    pub system_prompt: Option<String>,
    pub include_git_diff: bool,
    pub max_file_size_kb: u64,
    pub respect_gitignore: bool,
}

impl ContextBuilder {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            system_prompt: None,
            include_git_diff: false,
            max_file_size_kb: 512,
            respect_gitignore: true,
        }
    }

    /// Define o system prompt base.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Constrói o system prompt final, incluindo contexto do ambiente.
    pub async fn build_system_prompt(&self) -> anyhow::Result<String> {
        let mut parts = Vec::new();

        // System prompt base
        if let Some(ref sp) = self.system_prompt {
            parts.push(sp.clone());
        } else {
            parts.push(default_system_prompt());
        }

        // Informações do ambiente
        parts.push(self.environment_context().await?);

        Ok(parts.join("\n\n"))
    }

    async fn environment_context(&self) -> anyhow::Result<String> {
        let cwd = self.working_dir.display();
        let os = std::env::consts::OS;
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_owned());

        Ok(format!(
            "## Ambiente\n\
             - Sistema operacional: {os}\n\
             - Shell: {shell}\n\
             - Diretório de trabalho: {cwd}"
        ))
    }
}

fn default_system_prompt() -> String {
    r#"Você é o HyscodeCLI, um agente de codificação especializado.
Você tem acesso a ferramentas para ler e modificar arquivos, executar comandos e pesquisar código.
Seja preciso, eficiente e explique suas ações ao usuário.
Sempre verifique antes de fazer mudanças destrutivas.
Prefira soluções idiomáticas na linguagem do projeto.
Ao encontrar erros, analise a causa raiz antes de sugerir correções."#
        .to_owned()
}
