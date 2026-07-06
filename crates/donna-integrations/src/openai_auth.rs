use crate::browser::open_url;
use crate::secrets::SecretStore;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const ISSUER: &str = "https://auth.openai.com";
const CALLBACK_PORT: u16 = 1455;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAiOAuthTokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_at: i64,
    pub account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    id_token: String,
    expires_in: Option<i64>,
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Claims {
    chatgpt_account_id: Option<String>,
    #[serde(rename = "https://api.openai.com/auth")]
    openai_auth: Option<OpenAiAuthClaims>,
}

#[derive(Debug, Deserialize)]
struct OpenAiAuthClaims {
    chatgpt_account_id: Option<String>,
}

struct Pkce {
    verifier: String,
    challenge: String,
}

pub fn run_browser_login(secret_store: &dyn SecretStore, secret_ref: &str) -> Result<(), String> {
    let pkce = generate_pkce()?;
    let state = random_base64(32)?;
    let redirect = format!("http://localhost:{CALLBACK_PORT}/auth/callback");
    let listener = TcpListener::bind(("127.0.0.1", CALLBACK_PORT))
        .map_err(|error| format!("could not listen for OpenAI OAuth callback: {error}"))?;
    listener
        .set_nonblocking(false)
        .map_err(|error| error.to_string())?;

    let url = authorize_url(&redirect, &pkce, &state);
    println!("Opening browser for ChatGPT authorization: {url}");
    if let Err(error) = open_url(&url) {
        println!("{error}. Open this URL manually: {url}");
    }
    println!("Waiting for browser callback on http://localhost:{CALLBACK_PORT}/auth/callback ...");

    let code = wait_for_callback(listener, &state)?;
    let tokens = exchange_code(&code, &redirect, &pkce)?;
    store_openai_tokens(secret_store, secret_ref, &tokens)
        .map_err(|error| format!("could not store OpenAI token: {error}"))?;
    verify_secret_readback(secret_store, secret_ref)?;
    Ok(())
}

pub fn store_openai_tokens(
    secret_store: &dyn SecretStore,
    secret_ref: &str,
    tokens: &OpenAiOAuthTokenSet,
) -> Result<(), String> {
    let serialized = serde_json::to_string(tokens).map_err(|error| error.to_string())?;
    secret_store
        .set_secret(secret_ref, &serialized)
        .map_err(|error| error.to_string())
}

fn authorize_url(redirect: &str, pkce: &Pkce, state: &str) -> String {
    let params = [
        ("response_type", "code"),
        ("client_id", CLIENT_ID),
        ("redirect_uri", redirect),
        ("scope", "openid profile email offline_access"),
        ("code_challenge", pkce.challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", state),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "opencode"),
    ];
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{ISSUER}/oauth/authorize?{query}")
}

fn wait_for_callback(listener: TcpListener, expected_state: &str) -> Result<String, String> {
    listener
        .set_nonblocking(false)
        .map_err(|error| error.to_string())?;
    let (mut stream, _) = listener
        .accept()
        .map_err(|error| format!("failed to receive OAuth callback: {error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|error| error.to_string())?;
    let request = read_http_request(&mut stream)?;
    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| "empty OAuth callback request".to_owned())?;
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "invalid OAuth callback request".to_owned())?;
    let (_, query) = path
        .split_once('?')
        .ok_or_else(|| "OAuth callback had no query string".to_owned())?;
    let params = parse_query(query);

    if let Some(error) = params
        .get("error_description")
        .or_else(|| params.get("error"))
    {
        write_callback_response(&mut stream, false, error);
        return Err(error.clone());
    }
    if params.get("state").map(String::as_str) != Some(expected_state) {
        write_callback_response(&mut stream, false, "Invalid OAuth state");
        return Err("invalid OAuth state".to_owned());
    }
    let code = params
        .get("code")
        .cloned()
        .ok_or_else(|| "OAuth callback did not include a code".to_owned())?;
    write_callback_response(
        &mut stream,
        true,
        "Donna can now use ChatGPT auth. You may close this tab.",
    );
    Ok(code)
}

fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut buffer = [0u8; 4096];
    let mut request = Vec::new();
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") || request.len() > 16 * 1024 {
            break;
        }
    }
    String::from_utf8(request).map_err(|error| error.to_string())
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

fn exchange_code(code: &str, redirect: &str, pkce: &Pkce) -> Result<OpenAiOAuthTokenSet, String> {
    let response = Client::new()
        .post(format!("{ISSUER}/oauth/token"))
        .header("User-Agent", "donna/0.1.0")
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect),
            ("client_id", CLIENT_ID),
            ("code_verifier", pkce.verifier.as_str()),
        ])
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "OpenAI token exchange failed: {}",
            response.status()
        ));
    }
    let tokens = response
        .json::<TokenResponse>()
        .map_err(|error| error.to_string())?;
    Ok(OpenAiOAuthTokenSet {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: tokens.token_type.unwrap_or_else(|| "Bearer".to_owned()),
        expires_at: unix_now() + tokens.expires_in.unwrap_or(3600),
        account_id: extract_account_id(&tokens.id_token),
    })
}

fn generate_pkce() -> Result<Pkce, String> {
    let verifier = random_base64(32)?;
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Ok(Pkce {
        verifier,
        challenge,
    })
}

fn random_base64(length: usize) -> Result<String, String> {
    let mut bytes = vec![0u8; length];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn extract_account_id(id_token: &str) -> Option<String> {
    let payload = id_token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload.as_bytes()).ok()?;
    let claims = serde_json::from_slice::<Claims>(&decoded).ok()?;
    claims
        .chatgpt_account_id
        .or_else(|| claims.openai_auth.and_then(|auth| auth.chatgpt_account_id))
}

fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
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
            "OpenAI auth was written, but {secret_ref} could not be read back from OS secret storage"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{OpenAiOAuthTokenSet, parse_query, percent_decode, percent_encode};
    use crate::secrets::{InMemorySecretStore, SecretStore};

    #[test]
    fn percent_round_trip_handles_url_chars() {
        let value = "http://localhost:1455/auth/callback?x=a b";
        assert_eq!(percent_decode(&percent_encode(value)), value);
    }

    #[test]
    fn query_parser_decodes_callback_values() {
        let params = parse_query("code=abc%20123&state=ok");
        assert_eq!(params.get("code").map(String::as_str), Some("abc 123"));
        assert_eq!(params.get("state").map(String::as_str), Some("ok"));
    }

    #[test]
    fn tokens_store_as_json_secret() {
        let store = InMemorySecretStore::default();
        let tokens = OpenAiOAuthTokenSet {
            access_token: "access".to_owned(),
            refresh_token: "refresh".to_owned(),
            token_type: "Bearer".to_owned(),
            expires_at: 10,
            account_id: Some("account".to_owned()),
        };
        super::store_openai_tokens(&store, "donna/openai", &tokens).expect("store");
        let raw = store
            .get_secret("donna/openai")
            .expect("read")
            .expect("secret");
        assert!(raw.contains("refresh"));
        assert!(!raw.contains("donna.toml"));
    }
}
