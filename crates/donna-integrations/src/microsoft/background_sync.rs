use crate::microsoft::auth::{load_microsoft_tokens, store_microsoft_tokens};
use crate::microsoft::calendar::CALENDAR_SOURCE;
use crate::microsoft::error::GraphError;
use crate::microsoft::graph_client::{GraphSyncClient, mark_stale, shared_http_client};
use crate::microsoft::outlook::OUTLOOK_MAIL_SOURCE;
use crate::microsoft::teams::{TEAMS_CHANNEL_SOURCE, TEAMS_CHAT_SOURCE};
use crate::secrets::SecretStore;
use donna_config::AppConfig;
use donna_storage::LocalStore;
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
const TOKEN_URL_TEMPLATE: &str = "https://login.microsoftonline.com/{tenant_id}/oauth2/v2.0/token";

pub fn run_sync_once(
    store: &LocalStore,
    config: &AppConfig,
    secret_store: &dyn SecretStore,
) -> Result<(), GraphError> {
    eprintln!("donna microsoft sync: started");
    if store.is_offline()? {
        eprintln!("donna microsoft sync: skipped (offline)");
        for source in [
            OUTLOOK_MAIL_SOURCE,
            TEAMS_CHAT_SOURCE,
            TEAMS_CHANNEL_SOURCE,
            CALENDAR_SOURCE,
        ] {
            mark_stale(store, source, "offline")?;
        }
        return Ok(());
    }

    let Some(client_id) = config.microsoft.client_id.as_deref().filter(|id| !id.trim().is_empty()) else {
        eprintln!("donna microsoft sync: skipped (missing application id)");
        return Ok(());
    };
    let tenant_id = config.microsoft.tenant_id.trim();
    if tenant_id.is_empty() {
        eprintln!("donna microsoft sync: skipped (missing tenant id)");
        return Ok(());
    }

    let token_ref = config
        .microsoft
        .token_secret_ref
        .as_deref()
        .unwrap_or("donna/microsoft");
    let client_secret_ref = config
        .microsoft
        .client_secret_ref
        .as_deref()
        .unwrap_or("donna/microsoft-client-secret");
    let Some(client_secret) = secret_store.get_secret(client_secret_ref)? else {
        eprintln!(
            "donna microsoft sync: skipped (missing client secret at {client_secret_ref})"
        );
        return Ok(());
    };
    let Some(mut tokens) = load_microsoft_tokens(secret_store, token_ref)? else {
        eprintln!("donna microsoft sync: skipped (missing OAuth tokens at {token_ref})");
        return Ok(());
    };

    if is_expiring(tokens.expires_at) {
        eprintln!("donna microsoft sync: refreshing access token");
        let refreshed = refresh_tokens(
            tenant_id,
            client_id,
            &client_secret,
            tokens.refresh_token.as_deref().unwrap_or_default(),
            &config.microsoft.scopes,
        )?;
        tokens = refreshed;
        store_microsoft_tokens(secret_store, token_ref, &tokens)?;
        eprintln!("donna microsoft sync: access token refreshed");
    }

    let client = GraphSyncClient::new(&tokens, &config.microsoft);
    if let Err(error) = client.sync_all(store) {
        eprintln!("donna microsoft sync: failed: {error}");
        for source in [
            OUTLOOK_MAIL_SOURCE,
            TEAMS_CHAT_SOURCE,
            TEAMS_CHANNEL_SOURCE,
            CALENDAR_SOURCE,
        ] {
            let _ = mark_stale(store, source, &error.sync_error_message());
        }
        return Err(error);
    }

    eprintln!("donna microsoft sync: completed");
    Ok(())
}

fn refresh_tokens(
    tenant_id: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    scopes: &[String],
) -> Result<crate::microsoft::auth::MicrosoftTokenSet, GraphError> {
    if refresh_token.trim().is_empty() {
        return Err(GraphError::Auth(
            "stored Microsoft refresh token is missing".to_owned(),
        ));
    }
    let scope = scopes.join(" ");
    let response = shared_http_client()
        .post(TOKEN_URL_TEMPLATE.replace("{tenant_id}", tenant_id))
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token),
            ("scope", scope.as_str()),
        ])
        .send()?;
    let status = response.status();
    let body = response.text()?;
    if !status.is_success() {
        let parsed = serde_json::from_str::<OAuthError>(&body).ok();
        return Err(GraphError::auth_error(
            parsed
                .as_ref()
                .map(|error| error.error.as_str())
                .unwrap_or("token_refresh_failed"),
            parsed
                .as_ref()
                .and_then(|error| error.error_description.as_deref())
                .or(Some(body.as_str())),
        ));
    }
    let parsed: RefreshTokenResponse =
        serde_json::from_str(&body).map_err(|error| GraphError::UnexpectedResponse(error.to_string()))?;
    Ok(crate::microsoft::auth::MicrosoftTokenSet {
        access_token: parsed.access_token,
        refresh_token: parsed.refresh_token,
        token_type: parsed.token_type.unwrap_or_else(|| "Bearer".to_owned()),
        scope: parsed.scope,
        expires_at: parsed.expires_in.map(|seconds| now_seconds() + seconds),
    })
}

fn is_expiring(expires_at: Option<i64>) -> bool {
    match expires_at {
        Some(timestamp) => timestamp <= now_seconds() + 60,
        None => false,
    }
}


fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Deserialize)]
struct RefreshTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    error_description: Option<String>,
}
