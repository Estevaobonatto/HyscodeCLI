//! Registro de provedores — factory e seleção em runtime.

use std::{collections::HashMap, sync::Arc};
use hyscode_core::traits::provider::Provider;

/// Registro central de provedores configurados.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn Provider>>,
    default_provider: Option<String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: None,
        }
    }

    /// Registra um provedor com o nome dado.
    pub fn register(&mut self, name: impl Into<String>, provider: Arc<dyn Provider>) {
        self.providers.insert(name.into(), provider);
    }

    /// Define o provedor padrão.
    pub fn set_default(&mut self, name: impl Into<String>) {
        self.default_provider = Some(name.into());
    }

    /// Obtém um provedor pelo nome.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Provider>> {
        self.providers.get(name).cloned()
    }

    /// Obtém o provedor padrão.
    pub fn default_provider(&self) -> Option<Arc<dyn Provider>> {
        self.default_provider
            .as_deref()
            .and_then(|name| self.providers.get(name).cloned())
    }

    /// Lista todos os provedores registrados.
    pub fn list(&self) -> Vec<&str> {
        self.providers.keys().map(String::as_str).collect()
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}
