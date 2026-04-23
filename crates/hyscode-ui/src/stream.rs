//! Renderização de respostas em streaming no terminal.

use console::style;
use hyscode_core::models::response::ChatChunk;

/// Renderiza chunks de streaming diretamente no terminal.
pub struct StreamRenderer {
    buffer: String,
    show_token_count: bool,
}

impl StreamRenderer {
    pub fn new(_markdown_enabled: bool, show_token_count: bool) -> Self {
        Self {
            buffer: String::new(),
            show_token_count,
        }
    }

    /// Processa um chunk de resposta.
    pub fn on_chunk(&mut self, chunk: &ChatChunk) {
        if let Some(ref content) = chunk.delta.content {
            self.buffer.push_str(content);
            // Imprime diretamente (não espera o buffer completo para streaming real)
            print!("{}", content);
        }
    }

    /// Finaliza a renderização após o stream.
    pub fn finish(&self, usage: Option<&hyscode_core::models::usage::TokenUsage>) {
        println!(); // Nova linha após a resposta
        if self.show_token_count {
            if let Some(u) = usage {
                let info = format!(
                    "\n  {} prompt + {} completion = {} tokens",
                    u.prompt_tokens, u.completion_tokens, u.total_tokens
                );
                eprintln!("{}", style(info).dim());
            }
        }
    }

    /// Retorna o buffer acumulado (resposta completa).
    pub fn full_response(&self) -> &str {
        &self.buffer
    }
}
