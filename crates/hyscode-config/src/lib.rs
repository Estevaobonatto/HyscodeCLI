//! hyscode-config — Configurações e secrets

pub mod env;
pub mod file;
pub mod keyring;
pub mod vault;

pub use file::{load_config, save_config, Config};

// O vault local criptografado é o mecanismo padrão e único de armazenamento
// de secrets (substitui keyring em dev e produção).
pub use vault as secrets;
