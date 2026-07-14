use crate::browser::open_url;
use crate::microsoft::error::GraphError;
use crate::secrets::SecretStore;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use donna_config::AppConfig;
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DEFAULT_TOKEN_SECRET_REF: &str = "donna/microsoft";
const DEFAULT_CLIENT_SECRET_REF: &str = "donna/microsoft-client-secret";
const CALLBACK_PORT: u16 = 1467;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MicrosoftTokenSet {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub token_type: String,
    pub scope: Option<String>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ReqwestAuthCodeClient {
    client: Client,
    tenant_id: String,
}

impl ReqwestAuthCodeClient {
    pub fn new(tenant_id: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            tenant_id: tenant_id.into(),
        }
    }

    pub fn exchange_code(
        &self,
        client_id: &str,
        client_secret: &str,
        redirect_uri: &str,
        code: &str,
        code_verifier: &str,
    ) -> Result<MicrosoftTokenSet, GraphError> {
        let response = self
            .client
            .post(token_url(&self.tenant_id))
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", code_verifier),
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

    let application_id = prompt_required("Application id", config.microsoft.client_id.as_deref())?;
    let tenant_id = prompt_default("Tenant id", &config.microsoft.tenant_id)?;
    let client_secret = prompt_required("Client secret", None)?;
    let client_secret_ref = config
        .microsoft
        .client_secret_ref
        .clone()
        .unwrap_or_else(|| DEFAULT_CLIENT_SECRET_REF.to_owned());
    let token_ref = config
        .microsoft
        .token_secret_ref
        .clone()
        .unwrap_or_else(|| DEFAULT_TOKEN_SECRET_REF.to_owned());
    let redirect_uri = format!("http://localhost:{CALLBACK_PORT}/auth/callback");
    let pkce = generate_pkce()?;
    let state = random_base64(32)?;
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT)).map_err(|error| {
        GraphError::Auth(format!(
            "could not listen for Microsoft OAuth callback on {redirect_uri}: {error}"
        ))
    })?;

    config.microsoft.client_id = Some(application_id.clone());
    config.microsoft.tenant_id = tenant_id.clone();
    config.microsoft.client_secret_ref = Some(client_secret_ref.clone());
    config.microsoft.token_secret_ref = Some(token_ref.clone());
    config.save_to_path(config_path)?;
    secret_store.set_secret(&client_secret_ref, &client_secret)?;

    println!(
        "Saved Microsoft account metadata to {}. No token values were written to TOML.",
        config_path.display()
    );
    println!("Stored Microsoft client secret in OS secret storage at {client_secret_ref}.");

    let authorize_url = authorize_url(
        &tenant_id,
        &application_id,
        &config.microsoft.scopes,
        &redirect_uri,
        &pkce.challenge,
        &state,
    );
    println!("Opening browser for Microsoft authorization: {authorize_url}");
    if let Err(error) = open_url(&authorize_url) {
        println!("{error}. Open this URL manually: {authorize_url}");
    }
    println!("Waiting for browser callback on {redirect_uri} ...");

    let code = wait_for_callback(listener, &state)?;
    let client = ReqwestAuthCodeClient::new(tenant_id);
    let token = client.exchange_code(
        &application_id,
        &client_secret,
        &redirect_uri,
        &code,
        &pkce.verifier,
    )?;
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

fn parse_token_response(response: reqwest::blocking::Response) -> Result<MicrosoftTokenSet, GraphError> {
    let status = response.status();
    let body = response.text()?;

    if status.is_success() {
        let token: TokenSuccess = serde_json::from_str(&body)
            .map_err(|error| GraphError::UnexpectedResponse(error.to_string()))?;
        return token.into_token_set();
    }

    Err(parse_oauth_error(status, &body))
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

fn prompt(label: &str) -> Result<String, GraphError> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}

fn authorize_url(
    tenant_id: &str,
    client_id: &str,
    scopes: &[String],
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    let scope = scopes.join(" ");
    let params = [
        ("client_id", client_id),
        ("response_type", "code"),
        ("redirect_uri", redirect_uri),
        ("response_mode", "query"),
        ("scope", scope.as_str()),
        ("code_challenge", code_challenge),
        ("code_challenge_method", "S256"),
        ("state", state),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/authorize?{query}")
}

fn token_url(tenant_id: &str) -> String {
    format!("https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token")
}

fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String, GraphError> {
    let (mut stream, _) = listener
        .accept()
        .map_err(|error| GraphError::Auth(format!("failed to receive OAuth callback: {error}")))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .map_err(|error| GraphError::Auth(error.to_string()))?;
    let request = read_http_request(&mut stream)?;
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| GraphError::Auth("empty OAuth callback request".to_owned()))?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| GraphError::Auth("invalid OAuth callback request".to_owned()))?;
    let (_, query) = path
        .split_once('?')
        .ok_or_else(|| GraphError::Auth("OAuth callback had no query string".to_owned()))?;
    let params = parse_query(query);

    if let Some(error) = params
        .get("error_description")
        .or_else(|| params.get("error"))
    {
        write_callback_response(&mut stream, false, error);
        return Err(GraphError::Auth(error.clone()));
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        write_callback_response(&mut stream, false, "Invalid OAuth state");
        return Err(GraphError::Auth("invalid OAuth state".to_owned()));
    }
    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| GraphError::Auth("OAuth callback did not include a code".to_owned()))?;
    write_callback_response(
        &mut stream,
        true,
        "Donna can now use Microsoft Graph auth. You may close this tab.",
    );
    Ok(code)
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, GraphError> {
    let mut buffer = [0u8; 4096];
    let mut request = Vec::new();
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| GraphError::Auth(error.to_string()))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") || request.len() > 16 * 1024 {
            break;
        }
    }
    String::from_utf8(request).map_err(|error| GraphError::Auth(error.to_string()))
}

fn write_callback_response(stream: &mut TcpStream, success: bool, message: &str) {
    let title = if success {
        "Authorization complete"
    } else {
        "Authorization failed"
    };
    let status = if success { "200 OK" } else { "400 Bad Request" };
    let body = format!("<html><body><h1>{title}</h1><p>{message}</p></body></html>");
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

fn generate_pkce() -> Result<Pkce, GraphError> {
    let verifier = random_base64(32)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Ok(Pkce {
        verifier,
        challenge,
    })
}

fn random_base64(length: usize) -> Result<String, GraphError> {
    let mut bytes = vec![0u8; length];
    getrandom::fill(&mut bytes).map_err(|error| GraphError::Auth(error.to_string()))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            Some((percent_decode(key), percent_decode(value)))
        })
        .collect()
}

fn percent_encode(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&value[index + 1..index + 3], 16) {
                output.push(hex);
                index += 3;
                continue;
            }
        }
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn now_seconds() -> Result<i64, GraphError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(elapsed.as_secs() as i64)
}

struct Pkce {
    verifier: String,
    challenge: String,
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
