//! Módulo de adapters de provedores.

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "anthropic")]
pub mod anthropic;

#[cfg(feature = "copilot")]
pub mod copilot;

#[cfg(feature = "openrouter")]
pub mod openrouter;

#[cfg(feature = "zai")]
pub mod zai;

#[cfg(feature = "hyscode")]
pub mod hyscode;

#[cfg(feature = "gemini")]
pub mod gemini;
