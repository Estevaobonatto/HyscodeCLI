//! hyscode-provider — Adapters para provedores de LLM
//!
//! Implementa o trait `Provider` para cada provedor suportado.
//! Cada adapter é protegido por feature flag no Cargo.toml.

pub mod registry;
pub mod http;
pub mod adapters;

pub use registry::ProviderRegistry;
