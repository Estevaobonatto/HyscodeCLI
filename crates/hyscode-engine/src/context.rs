//! Construção do contexto para requisições ao modelo.
//!
//! Suporta:
//!  - System prompt customizado
//!  - Contexto de ambiente (OS, shell, cwd)
//!  - Resolução de menções `@caminho/arquivo` no texto do usuário
//!  - Injeção de git diff (`--staged` ou completo)
//!  - Respeito a .gitignore ao incluir arquivos

use std::path::{Path, PathBuf};
use tracing::warn;

/// Monta o contexto completo a ser enviado ao modelo.
#[derive(Debug, Clone)]
pub struct ContextBuilder {
    pub working_dir: PathBuf,
    pub system_prompt: Option<String>,
    pub include_git_diff: bool,
    pub git_diff_staged_only: bool,
    pub max_file_size_kb: u64,
    pub respect_gitignore: bool,
    /// Lista de arquivos/diretórios a incluir explicitamente como contexto.
    pub extra_files: Vec<PathBuf>,
}

impl ContextBuilder {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            system_prompt: None,
            include_git_diff: false,
            git_diff_staged_only: false,
            max_file_size_kb: 512,
            respect_gitignore: true,
            extra_files: Vec::new(),
        }
    }

    /// Define o system prompt base.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Ativa injeção de git diff no contexto.
    pub fn with_git_diff(mut self, staged_only: bool) -> Self {
        self.include_git_diff = true;
        self.git_diff_staged_only = staged_only;
        self
    }

    /// Adiciona arquivos/diretórios extras de contexto.
    pub fn with_extra_files(mut self, files: Vec<PathBuf>) -> Self {
        self.extra_files = files;
        self
    }

    /// Constrói o system prompt final, incluindo contexto do ambiente.
    pub async fn build_system_prompt(&self) -> anyhow::Result<String> {
        let mut parts = Vec::new();

        if let Some(ref sp) = self.system_prompt {
            parts.push(sp.clone());
        } else {
            parts.push(default_system_prompt());
        }

        parts.push(self.environment_context().await?);

        // Arquivos extras de contexto (--context flag da CLI)
        if !self.extra_files.is_empty() {
            let file_ctx = self.build_files_context(&self.extra_files).await;
            if !file_ctx.is_empty() {
                parts.push(file_ctx);
            }
        }

        // Git diff
        if self.include_git_diff {
            if let Some(diff) = self.get_git_diff().await {
                parts.push(diff);
            }
        }

        Ok(parts.join("\n\n"))
    }

    /// Extrai menções `@caminho` de uma string de texto e retorna:
    /// - O texto com as menções removidas.
    /// - O conteúdo dos arquivos mencionados formatado para o contexto.
    pub async fn resolve_at_mentions(&self, text: &str) -> (String, Option<String>) {
        let mut mentioned_paths: Vec<PathBuf> = Vec::new();
        let mut clean_words: Vec<&str> = Vec::new();

        for word in text.split_whitespace() {
            if let Some(mention) = word.strip_prefix('@') {
                let path = if Path::new(mention).is_absolute() {
                    PathBuf::from(mention)
                } else {
                    self.working_dir.join(mention)
                };
                mentioned_paths.push(path);
            } else {
                clean_words.push(word);
            }
        }

        if mentioned_paths.is_empty() {
            return (text.to_owned(), None);
        }

        let clean_text = clean_words.join(" ");
        let file_ctx = self.build_files_context(&mentioned_paths).await;
        let ctx = if file_ctx.is_empty() { None } else { Some(file_ctx) };

        (clean_text, ctx)
    }

    /// Lê e formata os arquivos da lista para inclusão no contexto.
    async fn build_files_context(&self, paths: &[PathBuf]) -> String {
        let mut sections = Vec::new();
        let max_bytes = self.max_file_size_kb * 1024;

        for path in paths {
            match tokio::fs::metadata(path).await {
                Ok(meta) if meta.is_dir() => {
                    if let Ok(dir_listing) = read_dir_shallow(path).await {
                        sections.push(format!(
                            "### Diretório: {}\n```\n{}\n```",
                            path.display(),
                            dir_listing
                        ));
                    }
                }
                Ok(meta) if meta.is_file() => {
                    if meta.len() > max_bytes {
                        sections.push(format!(
                            "### Arquivo: {} (muito grande — {} KB, máx {} KB)",
                            path.display(),
                            meta.len() / 1024,
                            self.max_file_size_kb
                        ));
                        continue;
                    }
                    match tokio::fs::read_to_string(path).await {
                        Ok(content) => {
                            let lang = lang_from_extension(path);
                            sections.push(format!(
                                "### Arquivo: {}\n```{}\n{}\n```",
                                path.display(),
                                lang,
                                content
                            ));
                        }
                        Err(e) => {
                            warn!("Não foi possível ler {}: {}", path.display(), e);
                        }
                    }
                }
                _ => {
                    warn!("Caminho não encontrado ou inacessível: {}", path.display());
                }
            }
        }

        if sections.is_empty() {
            return String::new();
        }

        format!("## Arquivos de Contexto\n\n{}", sections.join("\n\n"))
    }

    /// Obtém o git diff para injeção no contexto.
    async fn get_git_diff(&self) -> Option<String> {
        let args = if self.git_diff_staged_only {
            vec!["diff", "--staged"]
        } else {
            vec!["diff", "HEAD"]
        };

        let output = tokio::process::Command::new("git")
            .args(&args)
            .current_dir(&self.working_dir)
            .output()
            .await
            .ok()?;

        if !output.status.success() {
            return None;
        }

        let diff = String::from_utf8_lossy(&output.stdout).into_owned();
        if diff.trim().is_empty() {
            return None;
        }

        // Limita tamanho do diff
        let limited = if diff.len() > 16_000 {
            format!("{}\n... (diff truncado)", &diff[..16_000])
        } else {
            diff
        };

        Some(format!("## Git Diff\n\n```diff\n{}\n```", limited))
    }

    async fn environment_context(&self) -> anyhow::Result<String> {
        let cwd = self.working_dir.display();
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let shell = std::env::var("SHELL")
            .or_else(|_| std::env::var("COMSPEC"))
            .unwrap_or_else(|_| "unknown".to_owned());

        // Detectar se é um repositório git
        let git_info = get_git_branch(&self.working_dir).await;
        let git_line = match git_info {
            Some(branch) => format!("\n- Branch git: {}", branch),
            None => String::new(),
        };

        Ok(format!(
            "## Ambiente\n\
             - Sistema operacional: {os} ({arch})\n\
             - Shell: {shell}\n\
             - Diretório de trabalho: {cwd}{git_line}"
        ))
    }
}

async fn get_git_branch(dir: &Path) -> Option<String> {
    let out = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(dir)
        .output()
        .await
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        None
    }
}

async fn read_dir_shallow(dir: &Path) -> anyhow::Result<String> {
    let mut entries = tokio::fs::read_dir(dir).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        names.push(if is_dir { format!("{}/", name) } else { name });
    }
    names.sort();
    Ok(names.join("\n"))
}

fn lang_from_extension(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js" | "mjs") => "javascript",
        Some("ts") => "typescript",
        Some("go") => "go",
        Some("java") => "java",
        Some("c" | "h") => "c",
        Some("cpp" | "hpp" | "cc") => "cpp",
        Some("json") => "json",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        Some("md") => "markdown",
        Some("sh" | "bash") => "bash",
        Some("sql") => "sql",
        _ => "",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_resolve_at_mentions_no_mentions() {
        let builder = ContextBuilder::new(std::env::current_dir().unwrap());
        let (text, ctx) = builder.resolve_at_mentions("olá mundo sem menções").await;
        assert_eq!(text, "olá mundo sem menções");
        assert!(ctx.is_none());
    }

    #[tokio::test]
    async fn test_resolve_at_mentions_strips_tokens() {
        let builder = ContextBuilder::new(std::env::current_dir().unwrap());
        let (text, _ctx) = builder.resolve_at_mentions("refatora @src/main.rs e @README.md").await;
        assert_eq!(text, "refatora e");
    }

    #[test]
    fn test_lang_from_extension() {
        assert_eq!(lang_from_extension(Path::new("foo.rs")), "rust");
        assert_eq!(lang_from_extension(Path::new("bar.py")), "python");
        assert_eq!(lang_from_extension(Path::new("baz.unknown")), "");
    }
}
