//! Vault local criptografado para secrets.
//!
//! Substitui o keyring do SO por um arquivo local (`vault.enc`) protegido
//! por AES-256-GCM-SIV com chave derivada de entropia exclusiva da máquina
//! e do usuário. O arquivo só pode ser descriptografado no mesmo sistema.

use aes_gcm_siv::{
    aead::{Aead, KeyInit},
    Aes256GcmSiv, Nonce,
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, path::PathBuf};

const VAULT_FILE: &str = "vault.enc";
const NONCE_SIZE: usize = 12;

/// Mapa de secrets em memória.
#[derive(Debug, Default, Serialize, Deserialize)]
struct VaultData {
    secrets: HashMap<String, String>,
}

/// Retorna o diretório de configuração do HyscodeCLI.
fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("hyscode")
}

/// Retorna o caminho do arquivo de vault.
fn vault_path() -> PathBuf {
    config_dir().join(VAULT_FILE)
}

/// Coleta entropia do sistema para derivar a chave de criptografia.
///
/// Windows: MachineGuid do registry + nome do usuário + hostname.
/// Linux:   /etc/machine-id + nome do usuário + hostname.
/// macOS:   IOPlatformUUID (via ioreg/system_profiler fallback) + nome do usuário + hostname.
fn system_entropy() -> anyhow::Result<Vec<u8>> {
    let mut entropy = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // Entropia do ambiente Windows (sem dependências externas)
        for var in [
            "PROCESSOR_IDENTIFIER",
            "PROCESSOR_REVISION",
            "SystemRoot",
            "ProgramData",
        ] {
            if let Ok(v) = std::env::var(var) {
                entropy.extend_from_slice(v.as_bytes());
            }
        }
        // Volume serial do disco do sistema (via cmd, fallback silencioso)
        if let Ok(output) = std::process::Command::new("cmd")
            .args(["/C", "vol %SystemDrive%"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(serial) = stdout.split_whitespace().last() {
                entropy.extend_from_slice(serial.as_bytes());
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            entropy.extend_from_slice(id.trim().as_bytes());
        }
    }

    #[cfg(target_os = "macos")]
    {
        // Tenta obter IOPlatformUUID via ioreg
        if let Ok(output) = std::process::Command::new("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(start) = line.find('"') {
                        if let Some(end) = line[start + 1..].find('"') {
                            entropy.extend_from_slice(line[start + 1..start + 1 + end].as_bytes());
                        }
                    }
                    break;
                }
            }
        }
    }

    // Entropia comum a todos os SOs
    if let Ok(user) = std::env::var("USER").or_else(|_| std::env::var("USERNAME")) {
        entropy.extend_from_slice(user.as_bytes());
    }
    if let Ok(host) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
        entropy.extend_from_slice(host.as_bytes());
    }

    // Fallback mínimo: se nada foi coletado, usamos o diretório home como âncora
    if entropy.is_empty() {
        if let Some(home) = dirs::home_dir() {
            entropy.extend_from_slice(home.to_string_lossy().as_bytes());
        }
    }

    Ok(entropy)
}

/// Deriva uma chave de 32 bytes a partir da entropia do sistema.
fn derive_key() -> anyhow::Result<[u8; 32]> {
    let entropy = system_entropy().context("Falha ao coletar entropia do sistema")?;
    let hash = Sha256::digest(&entropy);
    let mut key = [0u8; 32];
    key.copy_from_slice(&hash);
    Ok(key)
}

/// Carrega o vault do disco e descriptografa.
fn load_vault() -> anyhow::Result<VaultData> {
    let path = vault_path();
    if !path.exists() {
        return Ok(VaultData::default());
    }

    let ciphertext = std::fs::read(&path).context("Falha ao ler vault.enc")?;
    if ciphertext.len() < NONCE_SIZE {
        anyhow::bail!("Arquivo de vault corrompido (muito curto)");
    }

    let (nonce_bytes, encrypted) = ciphertext.split_at(NONCE_SIZE);
    let nonce = Nonce::from_slice(nonce_bytes);

    let key = derive_key()?;
    let cipher = Aes256GcmSiv::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("Falha ao inicializar cipher: {:?}", e))?;

    let plaintext = cipher
        .decrypt(nonce, encrypted)
        .map_err(|e| anyhow::anyhow!("Falha ao descriptografar vault — entropia do sistema mudou ou arquivo corrompido: {:?}", e))?;

    let data: VaultData =
        serde_json::from_slice(&plaintext).context("Falha ao desserializar vault")?;
    Ok(data)
}

/// Salva o vault criptografado no disco.
fn save_vault(data: &VaultData) -> anyhow::Result<()> {
    let plaintext = serde_json::to_vec(data).context("Falha ao serializar vault")?;

    let key = derive_key()?;
    let cipher = Aes256GcmSiv::new_from_slice(&key)
        .map_err(|e| anyhow::anyhow!("Falha ao inicializar cipher: {:?}", e))?;

    let nonce_bytes: [u8; NONCE_SIZE] = rand::random();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_ref())
        .map_err(|e| anyhow::anyhow!("Falha ao criptografar vault: {:?}", e))?;

    let mut file_content = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    file_content.extend_from_slice(&nonce_bytes);
    file_content.extend_from_slice(&ciphertext);

    let path = vault_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).context("Falha ao criar diretório do vault")?;
    }
    std::fs::write(&path, &file_content).context("Falha ao escrever vault.enc")?;

    Ok(())
}

/// Armazena uma API key no vault local criptografado.
pub fn store_api_key(provider: &str, key: &str) -> anyhow::Result<()> {
    let mut vault = load_vault()?;
    vault.secrets.insert(provider.to_owned(), key.to_owned());
    save_vault(&vault)?;
    Ok(())
}

/// Recupera uma API key do vault local criptografado.
pub fn get_api_key(provider: &str) -> anyhow::Result<Option<String>> {
    let vault = load_vault()?;
    Ok(vault.secrets.get(provider).cloned())
}

/// Remove uma API key do vault.
pub fn delete_api_key(provider: &str) -> anyhow::Result<()> {
    let mut vault = load_vault()?;
    vault.secrets.remove(provider);
    save_vault(&vault)?;
    Ok(())
}
