//! hyscode-provider — Adapters para provedores de LLM
//!
//! Implementa o trait `Provider` para cada provedor suportado.
//! Cada adapter é protegido por feature flag no Cargo.toml.

pub mod adapters;
pub mod http;
pub mod registry;

pub use registry::ProviderRegistry;
