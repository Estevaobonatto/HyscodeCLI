//! Ferramentas: search_code, execute_command, git_diff.

use async_trait::async_trait;
use hyscode_core::{error::ToolError, models::tool::ToolResult, traits::tool::Tool};
use serde_json::{json, Value};
use std::path::Path;

// ── SearchCodeTool ───────────────────────────────────────────────────────────

pub struct SearchCodeTool;

#[async_trait]
impl Tool for SearchCodeTool {
    fn name(&self) -> &str { "search_code" }
    fn description(&self) -> &str {
        "Busca texto em arquivos do projeto. Retorna caminho:linha:conteúdo."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Texto para buscar" },
                "path": { "type": "string", "description": "Diretório raiz da busca (padrão: cwd)" },
                "file_glob": { "type": "string", "description": "Filtro de extensão, ex: .rs" }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let pattern = args["pattern"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'pattern' obrigatório".to_owned(),
        })?;
        let root = args["path"].as_str().unwrap_or(".").to_owned();
        let glob = args["file_glob"].as_str().map(|s| s.to_owned());
        let pattern_lower = pattern.to_lowercase();

        // Busca síncrona em blocking task
        let results = tokio::task::spawn_blocking(move || {
            let mut matches = Vec::new();
            search_recursive(Path::new(&root), &pattern_lower, glob.as_deref(), &mut matches)?;
            Ok::<Vec<String>, ToolError>(matches)
        })
        .await
        .map_err(|e| ToolError::Other(anyhow::anyhow!("task panicked: {e}")))?;

        let results = results?;
        if results.is_empty() {
            Ok(ToolResult::success("", "Nenhum resultado encontrado.".to_owned()))
        } else {
            let output = results.join("\n");
            // Limita output para não estourar contexto
            let limited = if output.len() > 8000 {
                format!("{}\n... (truncado, {} resultados)", &output[..8000], results.len())
            } else {
                output
            };
            Ok(ToolResult::success("", limited))
        }
    }
}

fn search_recursive(
    dir: &Path,
    pattern_lower: &str,
    glob: Option<&str>,
    matches: &mut Vec<String>,
) -> Result<(), ToolError> {
    if !dir.is_dir() {
        return Ok(());
    }

    let entries = std::fs::read_dir(dir).map_err(ToolError::Io)?;

    for entry in entries {
        let entry = entry.map_err(ToolError::Io)?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        // Skip diretórios ocultos e target/
        if name.starts_with('.') || name == "target" {
            continue;
        }

        if path.is_dir() {
            search_recursive(&path, pattern_lower, glob, matches)?;
        } else if path.is_file() {
            // Filtro de glob simples
            if let Some(g) = glob {
                if !name.ends_with(g.trim_start_matches('*')) {
                    continue;
                }
            }

            // Limita tamanho de arquivo
            if let Ok(meta) = path.metadata() {
                if meta.len() > 512 * 1024 {
                    continue; // skip arquivos > 512KB
                }
            }

            if let Ok(content) = std::fs::read_to_string(&path) {
                for (line_num, line) in content.lines().enumerate() {
                    if line.to_lowercase().contains(pattern_lower) {
                        let rel = path.strip_prefix(".")
                            .unwrap_or(&path)
                            .display()
                            .to_string()
                            .trim_start_matches("\\")
                            .trim_start_matches("/")
                            .to_owned();
                        matches.push(format!("{}:{}:{}", rel, line_num + 1, line.trim()));
                        if matches.len() >= 100 {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

// ── ExecuteCommandTool ───────────────────────────────────────────────────────

pub struct ExecuteCommandTool;

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str { "execute_command" }
    fn description(&self) -> &str {
        "Executa um comando shell e retorna stdout e stderr combinados."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Comando a executar" },
                "timeout_secs": { "type": "integer", "description": "Timeout em segundos (padrão: 30)" }
            },
            "required": ["command"]
        })
    }
    fn requires_confirmation(&self) -> bool { true }
    fn is_destructive(&self) -> bool { true }

    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let command = args["command"].as_str().ok_or_else(|| ToolError::InvalidArgs {
            tool: self.name().to_owned(),
            reason: "campo 'command' obrigatório".to_owned(),
        })?;
        let timeout_secs = args["timeout_secs"].as_u64().unwrap_or(30).min(300);

        // Rejeita comandos que contenham tentativas de escalonamento de privilégios.
        let blocked = ["sudo", "su ", "su\t", "pkexec", "doas", "runas"];
        let cmd_lower = command.to_lowercase();
        if blocked.iter().any(|b| cmd_lower.contains(b)) {
            return Err(ToolError::PermissionDenied(
                "comandos de escalonamento de privilégios não são permitidos.".to_owned(),
            ));
        }

        // Usa shell nativo para suportar pipes/redirecionamentos, mas com timeout fixo.
        let (shell, shell_flag) = if cfg!(windows) {
            ("cmd", "/C")
        } else {
            ("sh", "-c")
        };

        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            tokio::process::Command::new(shell)
                .arg(shell_flag)
                .arg(command)
                .output(),
        )
        .await
        .map_err(|_| ToolError::Timeout(self.name().to_owned()))?
        .map_err(ToolError::Io)?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let result = format!(
            "Exit code: {exit_code}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );

        if output.status.success() {
            Ok(ToolResult::success("", result))
        } else {
            Ok(ToolResult::error("", result))
        }
    }
}

// ── GitDiffTool ──────────────────────────────────────────────────────────────

pub struct GitDiffTool;

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str { "git_diff" }
    fn description(&self) -> &str {
        "Retorna o diff do repositório git. Por padrão retorna staged + unstaged changes."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "staged_only": { "type": "boolean", "description": "Retornar apenas staged changes" }
            }
        })
    }
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError> {
        let staged_only = args["staged_only"].as_bool().unwrap_or(false);
        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("diff");
        if staged_only { cmd.arg("--cached"); }

        let output = cmd.output().await.map_err(ToolError::Io)?;
        let diff = String::from_utf8_lossy(&output.stdout).into_owned();

        if diff.is_empty() {
            Ok(ToolResult::success("", "Nenhuma mudança encontrada."))
        } else {
            Ok(ToolResult::success("", diff))
        }
    }
}

// ---------------------------------------------------------------------------
// Testes
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_code_finds_text() {
        // Cria estrutura temporária
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.txt");
        std::fs::write(&file, "hello world\nfoo bar\nhello again\n").unwrap();

        let mut matches = Vec::new();
        search_recursive(tmp.path(), "hello", None, &mut matches).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(matches[0].contains("hello world"));
        assert!(matches[1].contains("hello again"));
    }

    #[test]
    fn test_search_code_respects_glob() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.rs"), "fn main() {}").unwrap();
        std::fs::write(tmp.path().join("b.txt"), "fn main() {}").unwrap();

        let mut matches = Vec::new();
        search_recursive(tmp.path(), "fn main", Some(".rs"), &mut matches).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].contains("a.rs"));
    }

    #[tokio::test]
    async fn test_execute_command_echo() {
        let tool = ExecuteCommandTool;
        let result = tool.execute(json!({"command": "echo hello"})).await.unwrap();
        assert!(result.content.contains("hello"));
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_git_diff_no_repo() {
        let tool = GitDiffTool;
        let result = tool.execute(json!({})).await.unwrap();
        // Fora de repo git, git diff retorna erro mas não panic
        assert!(!result.content.is_empty());
    }
}
