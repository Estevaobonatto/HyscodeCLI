//! Integração com o keyring do sistema operacional.
//!
//! - Windows: Windows Credential Manager
//! - macOS: Keychain
//! - Linux: Secret Service (libsecret) ou fallback para arquivo cifrado

const SERVICE_NAME: &str = "hyscode-cli";

/// Armazena uma API key no keyring do SO.
pub fn store_api_key(provider: &str, key: &str) -> anyhow::Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, provider)?;
    entry.set_password(key)?;
    Ok(())
}

/// Recupera uma API key do keyring do SO.
pub fn get_api_key(provider: &str) -> anyhow::Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, provider)?;
    match entry.get_password() {
        Ok(key) => Ok(Some(key)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("Erro ao acessar keyring: {}", e)),
    }
}

/// Remove uma API key do keyring.
pub fn delete_api_key(provider: &str) -> anyhow::Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, provider)?;
    entry.delete_credential()?;
    Ok(())
}
