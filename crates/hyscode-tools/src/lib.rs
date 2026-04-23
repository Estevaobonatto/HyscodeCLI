//! hyscode-tools — Ferramentas disponíveis ao agente

pub mod execute_command;
pub mod git_diff;
pub mod list_dir;
pub mod read_file;
pub mod registry;
pub mod search_code;
pub mod write_file;

pub use registry::ToolRegistry;
