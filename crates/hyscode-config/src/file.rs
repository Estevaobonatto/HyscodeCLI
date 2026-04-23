//! Leitura e escrita do arquivo de configuração TOML.

use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

/// Configuração raiz da aplicação.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub profile: ProfileConfig,
    #[serde(default)]
    pub ui: UiConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub context: ContextConfig,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub name: String,
    pub default_provider: String,
    pub default_model: String,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            name: "default".to_owned(),
            default_provider: "hyscode".to_owned(),
            default_model: "hyscode-smart".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub stream: bool,
    pub markdown: bool,
    pub syntax_highlight: bool,
    pub show_token_count: bool,
    pub show_cost: bool,
    pub interactive: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".to_owned(),
            stream: true,
            markdown: true,
            syntax_highlight: true,
            show_token_count: true,
            show_cost: false,
            interactive: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub auto_approve: bool,
    pub audit_only: bool,
    pub max_iterations: u32,
    pub confirm_writes: bool,
    pub confirm_commands: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            auto_approve: false,
            audit_only: false,
            max_iterations: 15,
            confirm_writes: true,
            confirm_commands: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    pub include_git_diff: bool,
    pub max_file_size_kb: u64,
    pub respect_gitignore: bool,
    pub custom_ignore: Vec<String>,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            include_git_diff: false,
            max_file_size_kb: 512,
            respect_gitignore: true,
            custom_ignore: vec![
                ".hyscode/".to_owned(),
                "*.lock".to_owned(),
                "target/".to_owned(),
            ],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key_source: ApiKeySource,
    pub env_var: Option<String>,
    pub base_url: Option<String>,
    pub default_model: String,
    pub timeout_secs: u64,
    pub max_retries: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ApiKeySource {
    #[default]
    Keyring,
    Env,
}

/// Retorna o caminho padrão do arquivo de configuração.
pub fn config_path() -> PathBuf {
    config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hyscode")
        .join("config.toml")
}

/// Carrega a configuração do disco.
pub fn load_config() -> anyhow::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = std::fs::read_to_string(&path)?;
    let config: Config = toml::from_str(&raw)?;
    Ok(config)
}

/// Salva a configuração no disco.
pub fn save_config(config: &Config) -> anyhow::Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let raw = toml::to_string_pretty(config)?;
    std::fs::write(&path, raw)?;
    Ok(())
}
