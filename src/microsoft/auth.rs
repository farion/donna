use crate::config::AppConfig;
use crate::microsoft::error::GraphError;
use crate::secrets::SecretStore;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DEVICE_CODE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const DEFAULT_TOKEN_SECRET_REF: &str = "donna/microsoft";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    #[serde(alias = "verification_url")]
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub expires_in: u64,
    pub interval: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MicrosoftTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReqwestDeviceCodeClient {
    client: Client,
    tenant_id: String,
}

impl ReqwestDeviceCodeClient {
    pub fn new(tenant_id: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            tenant_id: tenant_id.into(),
        }
    }

    pub fn request_device_code(
        &self,
        client_id: &str,
        scopes: &[String],
    ) -> Result<DeviceCodeResponse, GraphError> {
        let scope = scopes.join(" ");
        let response = self
            .client
            .post(device_code_url(&self.tenant_id))
            .form(&[("client_id", client_id), ("scope", scope.as_str())])
            .send()?;

        parse_device_code_response(response)
    }

    pub fn poll_for_token(
        &self,
        client_id: &str,
        device_code: &DeviceCodeResponse,
    ) -> Result<MicrosoftTokenSet, GraphError> {
        let mut interval = Duration::from_secs(device_code.interval.unwrap_or(5).max(1));
        let expires_at = Instant::now() + Duration::from_secs(device_code.expires_in);

        loop {
            if Instant::now() >= expires_at {
                return Err(GraphError::Auth("device code expired".to_owned()));
            }

            thread::sleep(interval);
            match self.poll_once(client_id, &device_code.device_code)? {
                PollOutcome::Token(token) => return Ok(token),
                PollOutcome::Pending => {}
                PollOutcome::SlowDown => interval += Duration::from_secs(5),
            }
        }
    }

    fn poll_once(&self, client_id: &str, device_code: &str) -> Result<PollOutcome, GraphError> {
        let response = self
            .client
            .post(token_url(&self.tenant_id))
            .form(&[
                ("grant_type", DEVICE_CODE_GRANT),
                ("client_id", client_id),
                ("device_code", device_code),
            ])
            .send()?;

        parse_token_response(response)
    }
}

pub fn run_auth_wizard(
    config_path: impl AsRef<Path>,
    secret_store: &dyn SecretStore,
) -> Result<(), GraphError> {
    let config_path = config_path.as_ref();
    let mut config = AppConfig::load_or_create_at(config_path)?;

    let client_id = prompt_required(
        "Microsoft app client id",
        config.microsoft.client_id.as_deref(),
    )?;
    let tenant_id = prompt_default("Tenant id", &config.microsoft.tenant_id)?;
    let account_hint = prompt_optional("Account hint", config.microsoft.account_hint.as_deref())?;
    let token_ref = prompt_default(
        "Token secret reference",
        config
            .microsoft
            .token_secret_ref
            .as_deref()
            .unwrap_or(DEFAULT_TOKEN_SECRET_REF),
    )?;

    config.microsoft.client_id = Some(client_id.clone());
    config.microsoft.tenant_id = tenant_id.clone();
    config.microsoft.account_hint = account_hint;
    config.microsoft.token_secret_ref = Some(token_ref.clone());
    config.save_to_path(config_path)?;

    println!(
        "Saved Microsoft account metadata to {}. No token values were written to TOML.",
        config_path.display()
    );

    let client = ReqwestDeviceCodeClient::new(tenant_id);
    let code = client.request_device_code(&client_id, &config.microsoft.scopes)?;
    println!("{}", code.message);
    println!("Verification URI: {}", code.verification_uri);
    println!("User code: {}", code.user_code);

    let token = client.poll_for_token(&client_id, &code)?;
    store_microsoft_tokens(secret_store, &token_ref, &token)?;
    println!("Stored Microsoft Graph tokens in OS secret storage at {token_ref}.");
    Ok(())
}

pub fn store_microsoft_tokens(
    secret_store: &dyn SecretStore,
    reference: &str,
    tokens: &MicrosoftTokenSet,
) -> Result<(), GraphError> {
    let serialized =
        serde_json::to_string(tokens).map_err(|error| GraphError::Auth(error.to_string()))?;
    secret_store.set_secret(reference, &serialized)?;
    Ok(())
}

pub fn load_microsoft_tokens(
    secret_store: &dyn SecretStore,
    reference: &str,
) -> Result<Option<MicrosoftTokenSet>, GraphError> {
    let Some(serialized) = secret_store.get_secret(reference)? else {
        return Ok(None);
    };

    serde_json::from_str(&serialized)
        .map(Some)
        .map_err(|error| {
            GraphError::Auth(format!("stored Microsoft token JSON is invalid: {error}"))
        })
}

fn parse_device_code_response(
    response: reqwest::blocking::Response,
) -> Result<DeviceCodeResponse, GraphError> {
    let status = response.status();
    let body = response.text()?;

    if status.is_success() {
        return serde_json::from_str(&body)
            .map_err(|error| GraphError::UnexpectedResponse(error.to_string()));
    }

    Err(parse_oauth_error(status, &body))
}

fn parse_token_response(response: reqwest::blocking::Response) -> Result<PollOutcome, GraphError> {
    let status = response.status();
    let body = response.text()?;

    if status.is_success() {
        let token: TokenSuccess = serde_json::from_str(&body)
            .map_err(|error| GraphError::UnexpectedResponse(error.to_string()))?;
        return Ok(PollOutcome::Token(token.into_token_set()?));
    }

    let error: OAuthError = serde_json::from_str(&body).unwrap_or_else(|_| OAuthError {
        error: status.to_string(),
        error_description: Some(body),
    });

    match error.error.as_str() {
        "authorization_pending" => Ok(PollOutcome::Pending),
        "slow_down" => Ok(PollOutcome::SlowDown),
        "authorization_declined" => Err(GraphError::Auth("authorization declined".to_owned())),
        "expired_token" => Err(GraphError::Auth("device code expired".to_owned())),
        _ => Err(GraphError::auth_error(
            &error.error,
            error.error_description.as_deref(),
        )),
    }
}

fn parse_oauth_error(status: StatusCode, body: &str) -> GraphError {
    let error: OAuthError = serde_json::from_str(body).unwrap_or_else(|_| OAuthError {
        error: status.to_string(),
        error_description: Some(body.to_owned()),
    });
    GraphError::auth_error(&error.error, error.error_description.as_deref())
}

fn prompt_required(label: &str, default: Option<&str>) -> Result<String, GraphError> {
    let value = match default {
        Some(default) => prompt_default(label, default)?,
        None => prompt(label)?,
    };

    if value.trim().is_empty() {
        return Err(GraphError::Auth(format!("{label} is required")));
    }

    Ok(value)
}

fn prompt_default(label: &str, default: &str) -> Result<String, GraphError> {
    let value = prompt(&format!("{label} [{default}]"))?;
    if value.trim().is_empty() {
        Ok(default.to_owned())
    } else {
        Ok(value)
    }
}

fn prompt_optional(label: &str, default: Option<&str>) -> Result<Option<String>, GraphError> {
    let prompt_label = match default {
        Some(default) => format!("{label} [{default}]"),
        None => label.to_owned(),
    };
    let value = prompt(&prompt_label)?;

    if value.trim().is_empty() {
        Ok(default.map(str::to_owned))
    } else {
        Ok(Some(value))
    }
}

fn prompt(label: &str) -> Result<String, GraphError> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}

fn device_code_url(tenant_id: &str) -> String {
    format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/devicecode")
}

fn token_url(tenant_id: &str) -> String {
    format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token")
}

fn now_seconds() -> Result<i64, GraphError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(elapsed.as_secs() as i64)
}

enum PollOutcome {
    Pending,
    SlowDown,
    Token(MicrosoftTokenSet),
}

#[derive(Deserialize)]
struct OAuthError {
    error: String,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct TokenSuccess {
    access_token: String,
    refresh_token: Option<String>,
    token_type: String,
    scope: Option<String>,
    expires_in: Option<i64>,
}

impl TokenSuccess {
    fn into_token_set(self) -> Result<MicrosoftTokenSet, GraphError> {
        let expires_at = match self.expires_in {
            Some(seconds) => Some(now_seconds()? + seconds),
            None => None,
        };

        Ok(MicrosoftTokenSet {
            access_token: self.access_token,
            refresh_token: self.refresh_token,
            token_type: self.token_type,
            scope: self.scope,
            expires_at,
        })
    }
}
