use crate::model::{ModelDefinition, ModelRegistry};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};

mod ollama;

pub use ollama::{OllamaChatRequest, OllamaProvider, WireChatMessage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFamily {
    Ollama,
    OpenAiCompatible,
    GithubCopilotCompatible,
    Mock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub requires_secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub model: ModelDefinition,
    pub family: ProviderFamily,
    pub capabilities: ProviderCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentTrust {
    Trusted,
    UntrustedExternal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiMessage {
    pub role: AiRole,
    pub content: String,
    pub trust: ContentTrust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiRequest {
    pub system_prompt: String,
    pub messages: Vec<AiMessage>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiResponse {
    pub text: String,
    pub provider: ProviderFamily,
    pub model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    NoConfiguredModels,
    UnknownModel(String),
    UnsupportedProvider(String),
    MissingBaseUrl(String),
    ProviderUnavailable {
        provider: ProviderFamily,
        detail: String,
    },
}

pub trait AiProvider {
    fn family(&self) -> ProviderFamily;

    fn complete(&self, model: &ModelDefinition, request: &AiRequest)
    -> Result<AiResponse, AiError>;
}

pub struct ProviderCatalog;

impl ProviderCatalog {
    pub fn select_chat_model(
        registry: &ModelRegistry,
        selected_model_id: &str,
    ) -> Result<ProviderSelection, AiError> {
        let model = registry
            .selected_or_first(selected_model_id)
            .ok_or(AiError::NoConfiguredModels)?;
        Self::selection_for_model(model)
    }

    pub fn select_task_model(
        registry: &ModelRegistry,
        default_model_id: &str,
        task_override: Option<&str>,
    ) -> Result<ProviderSelection, AiError> {
        let model = registry
            .task_model(default_model_id, task_override)
            .ok_or(AiError::NoConfiguredModels)?;
        Self::selection_for_model(model)
    }

    fn selection_for_model(model: &ModelDefinition) -> Result<ProviderSelection, AiError> {
        let family = ProviderFamily::parse(&model.provider)?;
        if model.base_url.as_deref().unwrap_or_default().is_empty() {
            return Err(AiError::MissingBaseUrl(model.id.clone()));
        }

        Ok(ProviderSelection {
            model: model.clone(),
            family,
            capabilities: family.capabilities(model.secret_ref.is_some()),
        })
    }
}

impl ProviderFamily {
    pub fn parse(value: &str) -> Result<Self, AiError> {
        match value {
            "ollama" => Ok(Self::Ollama),
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            "github-copilot-compatible" => Ok(Self::GithubCopilotCompatible),
            "mock" => Ok(Self::Mock),
            other => Err(AiError::UnsupportedProvider(other.to_owned())),
        }
    }

    fn capabilities(self, requires_secret: bool) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            requires_secret,
        }
    }
}

impl AiMessage {
    pub fn trusted(role: AiRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            trust: ContentTrust::Trusted,
        }
    }

    pub fn untrusted_external(role: AiRole, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            trust: ContentTrust::UntrustedExternal,
        }
    }
}

impl AiRequest {
    pub fn new(system_prompt: impl Into<String>) -> Self {
        Self {
            system_prompt: system_prompt.into(),
            messages: Vec::new(),
            stream: true,
        }
    }

    pub fn with_message(mut self, message: AiMessage) -> Self {
        self.messages.push(message);
        self
    }
}

pub struct MockProvider {
    response: String,
}

impl MockProvider {
    pub fn new(response: impl Into<String>) -> Self {
        Self {
            response: response.into(),
        }
    }
}

impl AiProvider for MockProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::Mock
    }

    fn complete(
        &self,
        model: &ModelDefinition,
        _request: &AiRequest,
    ) -> Result<AiResponse, AiError> {
        Ok(AiResponse {
            text: self.response.clone(),
            provider: self.family(),
            model_id: model.id.clone(),
        })
    }
}

impl Display for AiError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::NoConfiguredModels => write!(formatter, "no AI models are configured"),
            AiError::UnknownModel(model_id) => write!(formatter, "unknown AI model: {model_id}"),
            AiError::UnsupportedProvider(provider) => {
                write!(formatter, "unsupported AI provider: {provider}")
            }
            AiError::MissingBaseUrl(model_id) => {
                write!(formatter, "AI model {model_id} is missing a base URL")
            }
            AiError::ProviderUnavailable { provider, detail } => {
                write!(formatter, "{provider:?} provider unavailable: {detail}")
            }
        }
    }
}

impl Error for AiError {}

#[cfg(test)]
mod tests {
    use super::{
        AiMessage, AiProvider, AiRequest, AiRole, ContentTrust, MockProvider, ProviderCatalog,
        ProviderFamily,
    };
    use crate::config::AppConfig;
    use crate::model::ModelRegistry;

    #[test]
    fn selects_configured_chat_provider_by_model_id() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);

        let selection =
            ProviderCatalog::select_chat_model(&registry, "openai-compatible").expect("selection");

        assert_eq!(selection.family, ProviderFamily::OpenAiCompatible);
        assert_eq!(selection.model.model, "gpt-4.1-mini");
        assert!(selection.capabilities.requires_secret);
    }

    #[test]
    fn task_model_uses_task_default_not_ui_selection() {
        let mut config = AppConfig::default();
        config.ai.chat.selected_model = "openai-compatible".to_owned();
        config.ai.tasks.default_model = "ollama-local".to_owned();
        let registry = ModelRegistry::from_config(&config);

        let selection =
            ProviderCatalog::select_task_model(&registry, &config.ai.tasks.default_model, None)
                .expect("task model");

        assert_eq!(selection.model.id, "ollama-local");
        assert_eq!(selection.family, ProviderFamily::Ollama);
    }

    #[test]
    fn task_override_selects_configured_task_model() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);

        let selection = ProviderCatalog::select_task_model(
            &registry,
            &config.ai.tasks.default_model,
            Some("github-copilot-compatible"),
        )
        .expect("task override");

        assert_eq!(selection.family, ProviderFamily::GithubCopilotCompatible);
    }

    #[test]
    fn mock_provider_completes_without_persisting_prompt_data() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);
        let model = registry.selected_or_first("ollama-local").expect("model");
        let provider = MockProvider::new("ok");
        let request =
            AiRequest::new("system").with_message(AiMessage::trusted(AiRole::User, "hello"));

        let response = provider.complete(model, &request).expect("response");

        assert_eq!(response.text, "ok");
        assert_eq!(response.model_id, "ollama-local");
        assert_eq!(request.messages[0].trust, ContentTrust::Trusted);
    }
}
