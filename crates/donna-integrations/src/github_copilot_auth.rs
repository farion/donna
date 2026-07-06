use crate::browser::open_url;
use crate::secrets::SecretStore;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const COPILOT_TOKEN_URL: &str = "https://api.github.com/copilot_internal/v2/token";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GithubCopilotTokenSet {
    pub github_access_token: String,
    pub copilot_token: String,
    pub token_type: String,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GithubTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CopilotTokenResponse {
    token: String,
    expires_at: Option<i64>,
}

pub fn run_device_login(secret_store: &dyn SecretStore, secret_ref: &str) -> Result<(), String> {
    let client = Client::new();
    let code = request_device_code(&client)?;
    let login_url = code
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| code.verification_uri.clone());
    println!("Opening browser for GitHub Copilot authorization: {login_url}");
    if let Err(error) = open_url(&login_url) {
        println!("{error}. Open this URL manually: {login_url}");
    }
    println!("Enter code in GitHub if needed: {}", code.user_code);
    println!("Waiting for GitHub authorization...");

    let github_access_token = poll_for_github_token(&client, &code)?;
    let copilot = request_copilot_token(&client, &github_access_token)?;
    let tokens = GithubCopilotTokenSet {
        github_access_token,
        copilot_token: copilot.token,
        token_type: "Bearer".to_owned(),
        expires_at: copilot.expires_at,
    };
    store_copilot_tokens(secret_store, secret_ref, &tokens)
        .map_err(|error| format!("could not store GitHub Copilot token: {error}"))?;
    verify_secret_readback(secret_store, secret_ref)?;
    Ok(())
}

pub fn store_copilot_tokens(
    secret_store: &dyn SecretStore,
    secret_ref: &str,
    tokens: &GithubCopilotTokenSet,
) -> Result<(), String> {
    let serialized = serde_json::to_string(tokens).map_err(|error| error.to_string())?;
    secret_store
        .set_secret(secret_ref, &serialized)
        .map_err(|error| error.to_string())
}

fn request_device_code(client: &Client) -> Result<DeviceCodeResponse, String> {
    let response = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .header("User-Agent", "donna/0.1.0")
        .form(&[("client_id", CLIENT_ID), ("scope", "read:user")])
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub device code request failed: {}",
            response.status()
        ));
    }
    response
        .json::<DeviceCodeResponse>()
        .map_err(|error| error.to_string())
}

fn poll_for_github_token(client: &Client, code: &DeviceCodeResponse) -> Result<String, String> {
    let expires_at = Instant::now() + Duration::from_secs(code.expires_in);
    let mut interval = Duration::from_secs(code.interval.unwrap_or(5).max(1));
    loop {
        if Instant::now() >= expires_at {
            return Err("GitHub device code expired".to_owned());
        }
        thread::sleep(interval);
        let response = client
            .post(TOKEN_URL)
            .header("Accept", "application/json")
            .header("User-Agent", "donna/0.1.0")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", code.device_code.as_str()),
                ("client_id", CLIENT_ID),
            ])
            .send()
            .map_err(|error| error.to_string())?;
        let status = response.status();
        let token = response
            .json::<GithubTokenResponse>()
            .map_err(|error| error.to_string())?;
        if let Some(access_token) = token.access_token {
            return Ok(access_token);
        }
        match token.error.as_deref() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += Duration::from_secs(5),
            Some(error) => {
                return Err(token
                    .error_description
                    .unwrap_or_else(|| format!("GitHub authorization failed: {error}")));
            }
            None if status != StatusCode::OK => {
                return Err(format!("GitHub token polling failed: {status}"));
            }
            None => return Err("GitHub token polling returned no token".to_owned()),
        }
    }
}

fn request_copilot_token(
    client: &Client,
    github_access_token: &str,
) -> Result<CopilotTokenResponse, String> {
    let response = client
        .get(COPILOT_TOKEN_URL)
        .header("Accept", "application/json")
        .header("User-Agent", "donna/0.1.0")
        .header("Authorization", format!("token {github_access_token}"))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "GitHub Copilot token request failed: {}. Check that Copilot is enabled for this GitHub account.",
            response.status()
        ));
    }
    response
        .json::<CopilotTokenResponse>()
        .map_err(|error| error.to_string())
}

#[allow(dead_code)]
fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn verify_secret_readback(secret_store: &dyn SecretStore, secret_ref: &str) -> Result<(), String> {
    match secret_store
        .get_secret(secret_ref)
        .map_err(|error| error.to_string())?
    {
        Some(value) if !value.trim().is_empty() => Ok(()),
        _ => Err(format!(
            "GitHub Copilot auth was written, but {secret_ref} could not be read back from OS secret storage"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{GithubCopilotTokenSet, store_copilot_tokens};
    use crate::secrets::{InMemorySecretStore, SecretStore};

    #[test]
    fn copilot_tokens_store_as_json_secret() {
        let store = InMemorySecretStore::default();
        let tokens = GithubCopilotTokenSet {
            github_access_token: "github".to_owned(),
            copilot_token: "copilot".to_owned(),
            token_type: "Bearer".to_owned(),
            expires_at: Some(10),
        };

        store_copilot_tokens(&store, "donna/github-copilot", &tokens).expect("store");

        let raw = store
            .get_secret("donna/github-copilot")
            .expect("read")
            .expect("secret");
        assert!(raw.contains("copilot"));
        assert!(raw.contains("github"));
    }
}
