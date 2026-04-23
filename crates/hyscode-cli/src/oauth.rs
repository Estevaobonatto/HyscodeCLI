//! OAuth — Fluxo de autenticação para GitHub Copilot.
//!
//! Implementa o GitHub Device Flow para CLI:
//! https://docs.github.com/en/developers/apps/building-oauth-apps/authorizing-oauth-apps#device-flow

use std::time::Duration;

use serde::Deserialize;

const GITHUB_DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const GITHUB_ACCESS_TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const GITHUB_CLIENT_ID: &str = "Iv1.abc123"; // Placeholder — substituir pelo client_id real do app OAuth

/// Inicia o fluxo de OAuth para GitHub Copilot.
pub async fn authenticate_copilot() -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    // 1. Solicita device code
    let device_resp = client
        .post(GITHUB_DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", GITHUB_CLIENT_ID),
            ("scope", "read:user read:org"),
        ])
        .send()
        .await?
        .json::<DeviceCodeResponse>()
        .await?;

    println!("\n🔐 Autenticação GitHub Copilot");
    println!("Acesse: {}", device_resp.verification_uri);
    println!("Digite o código: {}\n", device_resp.user_code);

    // 2. Poll pelo access token
    let mut interval = Duration::from_secs(device_resp.interval.unwrap_or(5));
    let device_code = device_resp.device_code;

    loop {
        tokio::time::sleep(interval).await;

        let token_resp = client
            .post(GITHUB_ACCESS_TOKEN_URL)
            .header("Accept", "application/json")
            .form(&[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", &device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await?
            .json::<AccessTokenResponse>()
            .await?;

        if let Some(token) = token_resp.access_token {
            println!("✅ Autenticado com sucesso!");
            return Ok(token);
        }

        if let Some(error) = token_resp.error {
            match error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += Duration::from_secs(5);
                }
                "expired_token" => {
                    anyhow::bail!("O código de verificação expirou. Tente novamente.");
                }
                _ => {
                    anyhow::bail!("Erro no OAuth: {}", error);
                }
            }
        }
    }
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[allow(dead_code)]
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    #[allow(dead_code)]
    token_type: Option<String>,
    #[allow(dead_code)]
    scope: Option<String>,
    error: Option<String>,
    #[allow(dead_code)]
    error_description: Option<String>,
}
