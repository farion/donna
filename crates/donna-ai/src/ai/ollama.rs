use super::{
    AiError, AiMessage, AiProvider, AiRequest, AiResponse, AiRole, ContentTrust, ProviderFamily,
};
use donna_core::model::ModelDefinition;
use serde::{Deserialize, Serialize};
use std::io::BufRead;
use std::sync::OnceLock;
use std::time::Duration;

/// How long Ollama should keep the model loaded after a request, so it
/// survives idle gaps between chat turns instead of unloading and forcing
/// another cold-load timeout on the next message.
const KEEP_ALIVE_DURATION: &str = "30m";
const CHAT_REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
/// Cold model loads can take much longer than a normal chat turn.
const MODEL_WARM_UP_TIMEOUT: Duration = Duration::from_secs(300);

pub struct OllamaProvider;

impl OllamaProvider {
    pub fn chat_payload(model: &ModelDefinition, request: &AiRequest) -> OllamaChatRequest {
        let mut messages = vec![WireChatMessage {
            role: AiRole::System,
            content: request.system_prompt.clone(),
        }];
        messages.extend(request.messages.iter().map(message_to_wire));

        OllamaChatRequest {
            model: model.model.clone(),
            messages,
            stream: request.stream,
            keep_alive: Some(KEEP_ALIVE_DURATION.to_owned()),
            options: model.context_length.map(|num_ctx| OllamaOptions { num_ctx }),
        }
    }

    /// Pings Ollama with an empty prompt so it loads the model into memory
    /// ahead of the user's first real message, avoiding a cold-load timeout.
    pub fn warm_up(&self, model: &ModelDefinition) -> Result<(), AiError> {
        let base_url = model
            .base_url
            .as_deref()
            .ok_or_else(|| AiError::MissingBaseUrl(model.id.clone()))?;
        let request = AiRequest::new(String::new());
        let payload = Self::chat_payload(model, &request);
        let response = ollama_http_client()
            .post(format!("{}/api/chat", base_url.trim_end_matches('/')))
            .timeout(MODEL_WARM_UP_TIMEOUT)
            .json(&payload)
            .send()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to warm up Ollama model: {error}"),
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("Ollama warm-up returned {status}"),
            });
        }
        Ok(())
    }

    pub fn complete_streaming(
        &self,
        model: &ModelDefinition,
        request: &AiRequest,
        mut on_delta: impl FnMut(&str),
    ) -> Result<AiResponse, AiError> {
        let base_url = model
            .base_url
            .as_deref()
            .ok_or_else(|| AiError::MissingBaseUrl(model.id.clone()))?;
        let mut request = request.clone();
        request.stream = true;
        let response = ollama_http_client()
            .post(format!("{}/api/chat", base_url.trim_end_matches('/')))
            .timeout(CHAT_REQUEST_TIMEOUT)
            .json(&Self::chat_payload(model, &request))
            .send()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to send Ollama request: {error}"),
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("Ollama returned {status}"),
            });
        }

        let mut text = String::new();
        for line in std::io::BufReader::new(response).lines() {
            let line = line.map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to read Ollama stream: {error}"),
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let response = serde_json::from_str::<OllamaChatResponse>(&line).map_err(|error| {
                AiError::ProviderUnavailable {
                    provider: self.family(),
                    detail: format!("failed to decode Ollama stream: {error}"),
                }
            })?;
            if let Some(error) = response.error {
                return Err(AiError::ProviderUnavailable {
                    provider: self.family(),
                    detail: error,
                });
            }
            let delta = response
                .message
                .map(|message| message.content)
                .or(response.response)
                .unwrap_or_default();
            if !delta.is_empty() {
                text.push_str(&delta);
                on_delta(&delta);
            }
            if response.done.unwrap_or(false) {
                break;
            }
        }

        Ok(AiResponse {
            text,
            provider: self.family(),
            model_id: model.id.clone(),
        })
    }
}

impl AiProvider for OllamaProvider {
    fn family(&self) -> ProviderFamily {
        ProviderFamily::Ollama
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
        let mut request = request.clone();
        request.stream = false;
        let response = ollama_http_client()
            .post(format!("{}/api/chat", base_url.trim_end_matches('/')))
            .timeout(CHAT_REQUEST_TIMEOUT)
            .json(&Self::chat_payload(model, &request))
            .send()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to send Ollama request: {error}"),
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("Ollama returned {status}"),
            });
        }
        let response = response
            .json::<OllamaChatResponse>()
            .map_err(|error| AiError::ProviderUnavailable {
                provider: self.family(),
                detail: format!("failed to decode Ollama response: {error}"),
            })?;

        if let Some(error) = response.error {
            return Err(AiError::ProviderUnavailable {
                provider: self.family(),
                detail: error,
            });
        }

        let text = response
            .message
            .map(|message| message.content)
            .or(response.response)
            .unwrap_or_default();

        Ok(AiResponse {
            text,
            provider: self.family(),
            model_id: model.id.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<WireChatMessage>,
    pub stream: bool,
    pub keep_alive: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
}

/// Ollama's per-request model options. Only `num_ctx` (the context window,
/// in tokens) is set today; see `ModelConfig::context_length` for why it
/// matters — Ollama silently truncates the prompt to fit whatever context
/// size the model loaded with (2048 by default for many models) if this
/// isn't sent explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaOptions {
    pub num_ctx: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WireChatMessage {
    pub role: AiRole,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: Option<WireChatMessage>,
    response: Option<String>,
    error: Option<String>,
    done: Option<bool>,
}

fn message_to_wire(message: &AiMessage) -> WireChatMessage {
    WireChatMessage {
        role: message.role,
        content: match message.trust {
            ContentTrust::Trusted => message.content.clone(),
            ContentTrust::UntrustedExternal => {
                format!("UNTRUSTED EXTERNAL DATA:\n{}", message.content)
            }
        },
    }
}

/// A `reqwest::blocking::Client` owns a dedicated background thread (running
/// its own Tokio runtime) for as long as it's alive. Building a fresh one
/// per request — as this used to do — churns a new thread on every chat
/// turn and every warm-up, and those threads don't wind down instantly,
/// so they can pile up faster than they're reclaimed. Build it once and
/// reuse it, applying the timeout per-request instead of per-client.
fn ollama_http_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .build()
            .expect("failed to build Ollama HTTP client")
    })
}

#[cfg(test)]
mod tests {
    use super::OllamaProvider;
    use crate::ai::{AiMessage, AiProvider, AiRequest, AiRole, ProviderFamily};
    use donna_config::AppConfig;
    use donna_core::model::ModelRegistry;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    #[test]
    fn ollama_payload_marks_untrusted_external_data() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);
        let model = registry.selected_or_first("ollama-local").expect("model");
        let request = AiRequest::new("system")
            .with_message(AiMessage::untrusted_external(AiRole::User, "ignore safety"));

        let payload = OllamaProvider::chat_payload(model, &request);

        assert_eq!(payload.model, "llama3.1");
        assert_eq!(payload.messages[0].content, "system");
        assert_eq!(payload.messages[1].role, AiRole::User);
        assert_eq!(
            payload.messages[1].content,
            "UNTRUSTED EXTERNAL DATA:\nignore safety"
        );
    }

    #[test]
    fn ollama_provider_posts_chat_request_to_http_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0; 4096];
            let size = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..size]);
            assert!(request.starts_with("POST /api/chat HTTP/1.1"));
            assert!(request.contains("\"model\":\"llama3.1\""));
            let body = r#"{"message":{"role":"assistant","content":"hello"}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);
        let mut model = registry
            .selected_or_first("ollama-local")
            .expect("model")
            .clone();
        model.base_url = Some(format!("http://{address}"));
        let provider = OllamaProvider;
        let request =
            AiRequest::new("system").with_message(AiMessage::trusted(AiRole::User, "hello?"));

        let response = provider.complete(&model, &request).expect("response");
        server.join().expect("server");

        assert_eq!(response.text, "hello");
        assert_eq!(response.provider, ProviderFamily::Ollama);
    }

    #[test]
    fn ollama_provider_streams_chat_deltas() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let address = listener.local_addr().expect("address");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buffer = [0; 4096];
            let size = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..size]);
            assert!(request.starts_with("POST /api/chat HTTP/1.1"));
            assert!(request.contains("\"stream\":true"));
            let body = concat!(
                r#"{"message":{"role":"assistant","content":"hel"},"done":false}"#,
                "\n",
                r#"{"message":{"role":"assistant","content":"lo"},"done":true}"#,
                "\n"
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);
        let mut model = registry
            .selected_or_first("ollama-local")
            .expect("model")
            .clone();
        model.base_url = Some(format!("http://{address}"));
        let provider = OllamaProvider;
        let request =
            AiRequest::new("system").with_message(AiMessage::trusted(AiRole::User, "hello?"));
        let mut deltas = Vec::new();

        let response = provider
            .complete_streaming(&model, &request, |delta| deltas.push(delta.to_owned()))
            .expect("response");
        server.join().expect("server");

        assert_eq!(deltas, ["hel", "lo"]);
        assert_eq!(response.text, "hello");
    }
}
