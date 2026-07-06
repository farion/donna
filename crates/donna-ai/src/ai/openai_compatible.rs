use super::{AiError, AiMessage, AiProvider, AiRequest, AiResponse, AiRole, ProviderFamily};
use donna_core::model::ModelDefinition;
use serde::{Deserialize, Serialize};

const CHATGPT_CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";

pub struct OpenAiCompatibleProvider {
    family: ProviderFamily,
    auth_material: String,
}

impl OpenAiCompatibleProvider {
    pub fn new(family: ProviderFamily, auth_material: impl Into<String>) -> Self {
        Self {
            family,
            auth_material: auth_material.into(),
        }
    }

    pub fn bearer_token(auth_material: &str) -> Option<String> {
        ParsedAuth::from_material(auth_material).map(|auth| auth.token)
    }

    pub fn chat_payload(model: &ModelDefinition, request: &AiRequest) -> OpenAiChatRequest {
        let mut messages = vec![OpenAiWireChatMessage {
            role: AiRole::System,
            content: request.system_prompt.clone(),
        }];
        messages.extend(request.messages.iter().map(message_to_wire));
        OpenAiChatRequest {
            model: model.model.clone(),
            messages,
            stream: false,
        }
    }
}

impl AiProvider for OpenAiCompatibleProvider {
    fn family(&self) -> ProviderFamily {
        self.family
    }

    fn complete(
        &self,
        model: &ModelDefinition,
        request: &AiRequest,
    ) -> Result<AiResponse, AiError> {
        let base_url = model
            .base_url
            .as_deref()
            .ok_or_else(|| AiError::MissingBaseUrl(model.id.clone()))?;
        let auth = ParsedAuth::from_material(&self.auth_material)
            .ok_or_else(|| AiError::MissingSecret(model.id.clone()))?;
        if self.family == ProviderFamily::OpenAiCompatible && auth.kind == AuthKind::OpenAiOAuth {
            return self.complete_chatgpt_codex_responses(model, request, &auth);
        }
        self.complete_chat(model, request, base_url, &auth.token)
    }
}

impl OpenAiCompatibleProvider {
    fn complete_chat(
        &self,
        model: &ModelDefinition,
        request: &AiRequest,
        base_url: &str,
        token: &str,
    ) -> Result<AiResponse, AiError> {
        let response = reqwest::blocking::Client::new()
            .post(format!(
                "{}/chat/completions",
                base_url.trim_end_matches('/')
            ))
            .bearer_auth(token)
            .json(&Self::chat_payload(model, request))
            .send()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to send chat request: {error}"),
            })?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to read chat response: {error}"),
            })?;
        if !status.is_success() {
            return Err(AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!(
                    "chat endpoint returned {status}: {}",
                    trim_error_body(&body)
                ),
            });
        }
        let decoded = serde_json::from_str::<OpenAiChatResponse>(&body).map_err(|error| {
            AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to decode chat response: {error}"),
            }
        })?;
        let text = decoded
            .choices
            .into_iter()
            .next()
            .map(|choice| choice.message.content)
            .unwrap_or_default();

        Ok(AiResponse {
            text,
            provider: self.family(),
            model_id: model.id.clone(),
        })
    }

    fn complete_chatgpt_codex_responses(
        &self,
        model: &ModelDefinition,
        request: &AiRequest,
        auth: &ParsedAuth,
    ) -> Result<AiResponse, AiError> {
        let client = reqwest::blocking::Client::new();
        let mut builder = client
            .post(CHATGPT_CODEX_RESPONSES_URL)
            .bearer_auth(&auth.token)
            .json(&Self::responses_payload(model, request));
        if let Some(account_id) = &auth.account_id {
            builder = builder.header("ChatGPT-Account-Id", account_id);
        }
        let response = builder
            .send()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to send ChatGPT Codex request: {error}"),
            })?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to read ChatGPT Codex response: {error}"),
            })?;
        if !status.is_success() {
            return Err(AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!(
                    "ChatGPT Codex endpoint returned {status}: {}",
                    trim_error_body(&body)
                ),
            });
        }
        let text = if body.trim_start().starts_with("data:") || body.contains("\nevent: ") {
            responses_stream_output_text(&body)
        } else {
            let decoded =
                serde_json::from_str::<OpenAiResponsesResponse>(&body).map_err(|error| {
                    AiError::ProviderUnavailable {
                        provider: self.family(),
                        detail: format!("failed to decode ChatGPT Codex response: {error}"),
                    }
                })?;
            decoded.output_text()
        };
        Ok(AiResponse {
            text,
            provider: self.family(),
            model_id: model.id.clone(),
        })
    }

    pub fn responses_payload(
        model: &ModelDefinition,
        request: &AiRequest,
    ) -> OpenAiResponsesRequest {
        OpenAiResponsesRequest {
            model: model.model.clone(),
            instructions: request.system_prompt.clone(),
            store: false,
            stream: true,
            input: request
                .messages
                .iter()
                .map(message_to_responses_input)
                .collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthKind {
    ApiKey,
    OpenAiOAuth,
    CopilotOAuth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedAuth {
    token: String,
    kind: AuthKind,
    account_id: Option<String>,
}

impl ParsedAuth {
    fn from_material(auth_material: &str) -> Option<Self> {
        let trimmed = auth_material.trim();
        if trimmed.is_empty() {
            return None;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(token) = string_json_field(&json, "copilot_token") {
                return Some(Self {
                    token,
                    kind: AuthKind::CopilotOAuth,
                    account_id: None,
                });
            }
            if let Some(token) = string_json_field(&json, "access_token") {
                return Some(Self {
                    token,
                    kind: AuthKind::OpenAiOAuth,
                    account_id: string_json_field(&json, "account_id"),
                });
            }
            return None;
        }
        Some(Self {
            token: trimmed.to_owned(),
            kind: AuthKind::ApiKey,
            account_id: None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiChatRequest {
    pub model: String,
    pub messages: Vec<OpenAiWireChatMessage>,
    pub stream: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiWireChatMessage {
    pub role: AiRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiResponsesRequest {
    pub model: String,
    pub instructions: String,
    pub store: bool,
    pub stream: bool,
    pub input: Vec<OpenAiResponsesInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiResponsesInput {
    pub role: AiRole,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChatResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponsesResponse {
    output_text: Option<String>,
    #[serde(default)]
    output: Vec<OpenAiResponseOutput>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseOutput {
    #[serde(default)]
    content: Vec<OpenAiResponseContent>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseContent {
    text: Option<String>,
}

impl OpenAiResponsesResponse {
    fn output_text(self) -> String {
        if let Some(text) = self.output_text {
            return text;
        }
        self.output
            .into_iter()
            .flat_map(|item| item.content)
            .filter_map(|content| content.text)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn message_to_wire(message: &AiMessage) -> OpenAiWireChatMessage {
    OpenAiWireChatMessage {
        role: message.role,
        content: message.content.clone(),
    }
}

fn message_to_responses_input(message: &AiMessage) -> OpenAiResponsesInput {
    OpenAiResponsesInput {
        role: message.role,
        content: message.content.clone(),
    }
}

fn string_json_field(json: &serde_json::Value, field: &str) -> Option<String> {
    json.get(field)
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn responses_stream_output_text(body: &str) -> String {
    let mut deltas = Vec::new();
    let mut done_text = None;
    for event in body
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter(|data| *data != "[DONE]")
        .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
    {
        match event.get("type").and_then(|value| value.as_str()) {
            Some("response.output_text.delta") => {
                if let Some(delta) = event.get("delta").and_then(|value| value.as_str()) {
                    deltas.push(delta.to_owned());
                }
            }
            Some("response.output_text.done") => {
                done_text = event
                    .get("text")
                    .and_then(|value| value.as_str())
                    .map(str::to_owned);
            }
            _ => {}
        }
    }
    if deltas.is_empty() {
        done_text.unwrap_or_default()
    } else {
        deltas.join("")
    }
}

fn trim_error_body(body: &str) -> String {
    const MAX_LEN: usize = 500;
    let trimmed = body.trim();
    if trimmed.len() <= MAX_LEN {
        return trimmed.to_owned();
    }
    format!("{}...", &trimmed[..MAX_LEN])
}

#[cfg(test)]
mod tests {
    use super::{
        AuthKind, OpenAiCompatibleProvider, OpenAiResponsesResponse, ParsedAuth,
        responses_stream_output_text,
    };
    use crate::{AiMessage, AiRequest, AiRole};
    use donna_core::model::ModelDefinition;

    #[test]
    fn bearer_token_accepts_api_key_or_oauth_json() {
        assert_eq!(
            OpenAiCompatibleProvider::bearer_token("sk-test").as_deref(),
            Some("sk-test")
        );
        assert_eq!(
            OpenAiCompatibleProvider::bearer_token(r#"{"access_token":"oauth"}"#).as_deref(),
            Some("oauth")
        );
        assert_eq!(
            OpenAiCompatibleProvider::bearer_token(r#"{"copilot_token":"copilot"}"#).as_deref(),
            Some("copilot")
        );
    }

    #[test]
    fn auth_material_tracks_token_kind() {
        assert_eq!(
            ParsedAuth::from_material("sk-test").map(|auth| auth.kind),
            Some(AuthKind::ApiKey)
        );
        assert_eq!(
            ParsedAuth::from_material(r#"{"access_token":"oauth"}"#).map(|auth| auth.kind),
            Some(AuthKind::OpenAiOAuth)
        );
        assert_eq!(
            ParsedAuth::from_material(r#"{"access_token":"oauth","account_id":"acc-123"}"#)
                .and_then(|auth| auth.account_id),
            Some("acc-123".to_owned())
        );
        assert_eq!(
            ParsedAuth::from_material(r#"{"copilot_token":"copilot"}"#).map(|auth| auth.kind),
            Some(AuthKind::CopilotOAuth)
        );
    }

    #[test]
    fn chat_payload_uses_system_prompt_first() {
        let model = ModelDefinition {
            id: "openai-compatible".to_owned(),
            label: "OpenAI compatible".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: "gpt-4.1-mini".to_owned(),
            base_url: Some("https://api.openai.com/v1".to_owned()),
            secret_ref: Some("donna/openai".to_owned()),
        };
        let request = AiRequest::new("system").with_message(AiMessage::trusted(AiRole::User, "hi"));

        let payload = OpenAiCompatibleProvider::chat_payload(&model, &request);

        assert_eq!(payload.messages[0].role, AiRole::System);
        assert_eq!(payload.messages[0].content, "system");
        assert_eq!(payload.messages[1].role, AiRole::User);
        assert!(!payload.stream);
    }

    #[test]
    fn responses_payload_uses_instructions_for_system_prompt() {
        let model = ModelDefinition {
            id: "openai-compatible".to_owned(),
            label: "OpenAI compatible".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: "gpt-5.5".to_owned(),
            base_url: Some("https://api.openai.com/v1".to_owned()),
            secret_ref: Some("donna/openai".to_owned()),
        };
        let request = AiRequest::new("system").with_message(AiMessage::trusted(AiRole::User, "hi"));

        let payload = OpenAiCompatibleProvider::responses_payload(&model, &request);

        assert_eq!(payload.model, "gpt-5.5");
        assert_eq!(payload.instructions, "system");
        assert!(!payload.store);
        assert!(payload.stream);
        assert_eq!(payload.input[0].role, AiRole::User);
        assert_eq!(payload.input[0].content, "hi");
    }

    #[test]
    fn responses_stream_output_text_joins_deltas() {
        let body = r#"event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"hel"}

event: response.output_text.delta
data: {"type":"response.output_text.delta","delta":"lo"}

event: response.output_text.done
data: {"type":"response.output_text.done","text":"hello"}

data: [DONE]
"#;

        assert_eq!(responses_stream_output_text(body), "hello");
    }

    #[test]
    fn responses_output_text_falls_back_to_output_items() {
        let decoded = serde_json::from_str::<OpenAiResponsesResponse>(
            r#"{"output":[{"content":[{"text":"hello"},{"text":"world"}]}]}"#,
        )
        .expect("decode");

        assert_eq!(decoded.output_text(), "hello\nworld");
    }
}
