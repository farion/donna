use crate::config::{AppConfig, ConfigError, ModelConfig};
use crate::microsoft;
use crate::secrets::{SecretError, SecretStore};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::{self, Write};
use std::path::Path;

const OLLAMA_MODEL_ID: &str = "ollama-local";
const OPENAI_MODEL_ID: &str = "openai-compatible";
const GITHUB_COPILOT_MODEL_ID: &str = "github-copilot-compatible";

#[derive(Debug)]
pub enum AuthWizardError {
    Config(ConfigError),
    Graph(microsoft::error::GraphError),
    Secret(SecretError),
    Io(io::Error),
    Message(String),
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthProviderChoice {
    Microsoft,
    OpenAi,
    Ollama,
    GithubCopilot,
}

pub fn run_auth_wizard(
    config_path: impl AsRef<Path>,
    secret_store: &dyn SecretStore,
) -> Result<(), AuthWizardError> {
    let config_path = config_path.as_ref();
    let choice = prompt_provider_choice()?;

    match choice {
        AuthProviderChoice::Microsoft => {
            microsoft::auth::run_auth_wizard(config_path, secret_store)?;
        }
        AuthProviderChoice::OpenAi => configure_secret_model(
            config_path,
            secret_store,
            SecretModelDefaults {
                id: OPENAI_MODEL_ID,
                label: "OpenAI compatible",
                provider: "openai-compatible",
                base_url: "https://api.openai.com/v1",
                model: "gpt-4.1-mini",
                secret_ref: "donna/openai",
                secret_label: "OpenAI API key",
            },
        )?,
        AuthProviderChoice::Ollama => configure_ollama(config_path)?,
        AuthProviderChoice::GithubCopilot => configure_secret_model(
            config_path,
            secret_store,
            SecretModelDefaults {
                id: GITHUB_COPILOT_MODEL_ID,
                label: "GitHub Copilot compatible",
                provider: "github-copilot-compatible",
                base_url: "https://api.githubcopilot.com",
                model: "gpt-4.1",
                secret_ref: "donna/github-copilot",
                secret_label: "GitHub Copilot API token",
            },
        )?,
    }

    Ok(())
}

struct SecretModelDefaults {
    id: &'static str,
    label: &'static str,
    provider: &'static str,
    base_url: &'static str,
    model: &'static str,
    secret_ref: &'static str,
    secret_label: &'static str,
}

fn configure_secret_model(
    config_path: &Path,
    secret_store: &dyn SecretStore,
    defaults: SecretModelDefaults,
) -> Result<(), AuthWizardError> {
    let mut config = AppConfig::load_or_create_at(config_path)?;
    let existing = model_by_id(&config, defaults.id);
    let base_url = prompt_default(
        "Base URL",
        existing
            .and_then(|model| model.base_url.as_deref())
            .unwrap_or(defaults.base_url),
    )?;
    let model_name = prompt_default(
        "Model",
        existing
            .map(|model| model.model.as_str())
            .unwrap_or(defaults.model),
    )?;
    let secret_ref = prompt_default(
        "Secret reference",
        existing
            .and_then(|model| model.secret_ref.as_deref())
            .unwrap_or(defaults.secret_ref),
    )?;
    let api_key = prompt_secret_required(defaults.secret_label)?;

    upsert_model(
        &mut config,
        ModelConfig {
            id: defaults.id.to_owned(),
            label: defaults.label.to_owned(),
            provider: defaults.provider.to_owned(),
            model: model_name,
            base_url: Some(base_url),
            secret_ref: Some(secret_ref.clone()),
        },
    );
    if prompt_yes_no("Use this provider as the selected chat model", true)? {
        config.ai.chat.selected_model = defaults.id.to_owned();
    }
    config.save_to_path(config_path)?;
    secret_store.set_secret(&secret_ref, &api_key)?;

    println!(
        "Saved {} model metadata to {} and stored the token in OS secret storage at {secret_ref}.",
        defaults.label,
        config_path.display()
    );
    Ok(())
}

fn configure_ollama(config_path: &Path) -> Result<(), AuthWizardError> {
    let mut config = AppConfig::load_or_create_at(config_path)?;
    let existing = model_by_id(&config, OLLAMA_MODEL_ID);
    let base_url = prompt_default(
        "Ollama base URL",
        existing
            .and_then(|model| model.base_url.as_deref())
            .unwrap_or("http://localhost:11434"),
    )?;
    let model_name = prompt_default(
        "Ollama model",
        existing
            .map(|model| model.model.as_str())
            .unwrap_or("llama3.1"),
    )?;

    upsert_model(
        &mut config,
        ModelConfig {
            id: OLLAMA_MODEL_ID.to_owned(),
            label: "Ollama local".to_owned(),
            provider: "ollama".to_owned(),
            model: model_name,
            base_url: Some(base_url),
            secret_ref: None,
        },
    );
    if prompt_yes_no("Use Ollama as the selected chat model", true)? {
        config.ai.chat.selected_model = OLLAMA_MODEL_ID.to_owned();
    }
    config.save_to_path(config_path)?;

    println!(
        "Saved Ollama model metadata to {}. No secret was required or stored.",
        config_path.display()
    );
    Ok(())
}

fn model_by_id<'a>(config: &'a AppConfig, id: &str) -> Option<&'a ModelConfig> {
    config.ai.models.iter().find(|model| model.id == id)
}

fn upsert_model(config: &mut AppConfig, model: ModelConfig) {
    if let Some(existing) = config
        .ai
        .models
        .iter_mut()
        .find(|existing| existing.id == model.id)
    {
        *existing = model;
    } else {
        config.ai.models.push(model);
    }
}

fn prompt_provider_choice() -> Result<AuthProviderChoice, AuthWizardError> {
    println!("Choose what to configure:");
    println!("  1) Microsoft Graph");
    println!("  2) OpenAI-compatible API");
    println!("  3) Ollama");
    println!("  4) GitHub Copilot-compatible API");

    loop {
        let value = prompt("Provider")?;
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "microsoft" | "ms" | "graph" => return Ok(AuthProviderChoice::Microsoft),
            "2" | "openai" | "openai-compatible" => return Ok(AuthProviderChoice::OpenAi),
            "3" | "ollama" => return Ok(AuthProviderChoice::Ollama),
            "4" | "github" | "copilot" | "github-copilot" => {
                return Ok(AuthProviderChoice::GithubCopilot);
            }
            "q" | "quit" | "cancel" => return Err(AuthWizardError::Cancelled),
            _ => println!("Enter 1, 2, 3, 4, or q to cancel."),
        }
    }
}

fn prompt_secret_required(label: &str) -> Result<String, AuthWizardError> {
    let value = rpassword::prompt_password(format!("{label}: "))?;
    if value.trim().is_empty() {
        return Err(AuthWizardError::Message(format!("{label} is required")));
    }
    Ok(value)
}

fn prompt_default(label: &str, default: &str) -> Result<String, AuthWizardError> {
    let value = prompt(&format!("{label} [{default}]"))?;
    if value.trim().is_empty() {
        Ok(default.to_owned())
    } else {
        Ok(value)
    }
}

fn prompt_yes_no(label: &str, default: bool) -> Result<bool, AuthWizardError> {
    let suffix = if default { "Y/n" } else { "y/N" };
    loop {
        let value = prompt(&format!("{label} [{suffix}]"))?;
        let trimmed = value.trim().to_ascii_lowercase();
        if trimmed.is_empty() {
            return Ok(default);
        }
        match trimmed.as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Enter y or n."),
        }
    }
}

fn prompt(label: &str) -> Result<String, AuthWizardError> {
    print!("{label}: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end_matches(['\r', '\n']).to_owned())
}

impl AuthWizardError {
    fn message(&self) -> Option<&str> {
        match self {
            AuthWizardError::Cancelled => Some("auth setup cancelled"),
            _ => None,
        }
    }
}

impl Display for AuthWizardError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = self.message() {
            return formatter.write_str(message);
        }
        match self {
            AuthWizardError::Config(source) => write!(formatter, "{source}"),
            AuthWizardError::Graph(source) => write!(formatter, "{source}"),
            AuthWizardError::Secret(source) => write!(formatter, "{source}"),
            AuthWizardError::Io(source) => write!(formatter, "auth setup IO error: {source}"),
            AuthWizardError::Message(message) => formatter.write_str(message),
            AuthWizardError::Cancelled => unreachable!(),
        }
    }
}

impl Error for AuthWizardError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            AuthWizardError::Config(source) => Some(source),
            AuthWizardError::Graph(source) => Some(source),
            AuthWizardError::Secret(source) => Some(source),
            AuthWizardError::Io(source) => Some(source),
            AuthWizardError::Message(_) => None,
            AuthWizardError::Cancelled => None,
        }
    }
}

impl From<ConfigError> for AuthWizardError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<microsoft::error::GraphError> for AuthWizardError {
    fn from(error: microsoft::error::GraphError) -> Self {
        Self::Graph(error)
    }
}

impl From<SecretError> for AuthWizardError {
    fn from(error: SecretError) -> Self {
        Self::Secret(error)
    }
}

impl From<io::Error> for AuthWizardError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::{OPENAI_MODEL_ID, upsert_model};
    use crate::config::{AppConfig, ModelConfig};

    #[test]
    fn upsert_model_replaces_existing_model() {
        let mut config = AppConfig::default();

        upsert_model(
            &mut config,
            ModelConfig {
                id: OPENAI_MODEL_ID.to_owned(),
                label: "OpenAI compatible".to_owned(),
                provider: "openai-compatible".to_owned(),
                model: "gpt-4.1".to_owned(),
                base_url: Some("https://example.test/v1".to_owned()),
                secret_ref: Some("donna/test-openai".to_owned()),
            },
        );

        let models: Vec<_> = config
            .ai
            .models
            .iter()
            .filter(|model| model.id == OPENAI_MODEL_ID)
            .collect();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].model, "gpt-4.1");
        assert_eq!(models[0].secret_ref.as_deref(), Some("donna/test-openai"));
    }
}
