//! hyscode-tools — Ferramentas disponíveis ao agente

pub mod registry;
pub mod read_file;
pub mod write_file;
pub mod list_dir;
pub mod search_code;
pub mod execute_command;
pub mod git_diff;

pub use registry::ToolRegistry;
