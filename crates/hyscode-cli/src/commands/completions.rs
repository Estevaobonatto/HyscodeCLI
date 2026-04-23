//! Gera scripts de autocompleção para shells suportados.

use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::Cli;

/// Gera e imprime o script de autocompleção para o shell especificado.
pub fn run(shell: Shell) {
    let mut cmd = Cli::command();
    generate(shell, &mut cmd, "hyscode", &mut io::stdout());
}
