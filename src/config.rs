use directories::BaseDirs;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub ui: UiConfig,
    pub avatar: AvatarConfig,
    pub ai: AiConfig,
    pub microsoft: MicrosoftConfig,
    pub notes: NotesConfig,
    pub prompts: PromptsConfig,
    pub data: DataConfig,
    pub tasks: TaskConfig,
    pub memory: MemoryConfig,
    pub attention: AttentionConfig,
    pub offline: OfflineConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct UiConfig {
    pub donna_message_color: String,
    pub user_message_color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AvatarConfig {
    pub character: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AiConfig {
    pub chat: ChatModelConfig,
    pub tasks: TaskModelConfig,
    pub models: Vec<ModelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ChatModelConfig {
    pub selected_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TaskModelConfig {
    pub default_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelConfig {
    pub id: String,
    pub label: String,
    pub provider: String,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MicrosoftConfig {
    pub client_id: Option<String>,
    pub tenant_id: String,
    pub scopes: Vec<String>,
    pub account_hint: Option<String>,
    pub token_secret_ref: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NotesConfig {
    pub obsidian_vault_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PromptsConfig {
    pub system_prompt_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DataConfig {
    pub database_path: PathBuf,
    pub stale_after_minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct TaskConfig {
    pub directory: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct MemoryConfig {
    pub require_sensitive_approval: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AttentionConfig {
    pub enabled: bool,
    pub notification_min_level: String,
    pub popup_min_level: String,
    pub popup_cooldown_seconds: u32,
    pub critical_bypasses_cooldown: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct OfflineConfig {
    pub show_stale_data_warnings: bool,
    pub queue_external_actions: bool,
}

#[derive(Debug)]
pub enum ConfigError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Decode {
        path: PathBuf,
        source: toml::de::Error,
    },
    Encode {
        source: toml::ser::Error,
    },
}

impl AppConfig {
    pub fn default_path() -> PathBuf {
        if let Some(base_dirs) = BaseDirs::new() {
            return base_dirs.config_dir().join("donna").join("donna.toml");
        }

        PathBuf::from("donna.toml")
    }

    pub fn load_or_create_at(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        if path.exists() {
            return Self::load_at(path);
        }

        let config = Self::default();
        config.save_to_path(path)?;
        Ok(config)
    }

    pub fn load_or_default_at(path: impl AsRef<Path>) -> (Self, Option<String>) {
        match Self::load_or_create_at(path) {
            Ok(config) => (config, None),
            Err(error) => (Self::default(), Some(error.to_string())),
        }
    }

    pub fn load_at(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|source| ConfigError::Io {
            path: path.to_owned(),
            source,
        })?;

        toml::from_str(&contents).map_err(|source| ConfigError::Decode {
            path: path.to_owned(),
            source,
        })
    }

    pub fn save_to_path(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
                path: parent.to_owned(),
                source,
            })?;
        }

        let contents =
            toml::to_string_pretty(self).map_err(|source| ConfigError::Encode { source })?;

        fs::write(path, contents).map_err(|source| ConfigError::Io {
            path: path.to_owned(),
            source,
        })
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            donna_message_color: "#eef5ff".to_owned(),
            user_message_color: "#eaf7ef".to_owned(),
        }
    }
}

impl Default for AvatarConfig {
    fn default() -> Self {
        Self {
            character: "donna".to_owned(),
        }
    }
}

impl Default for MicrosoftConfig {
    fn default() -> Self {
        Self {
            client_id: None,
            tenant_id: "common".to_owned(),
            scopes: default_microsoft_scopes(),
            account_hint: None,
            token_secret_ref: None,
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        let models = default_models();
        Self {
            chat: ChatModelConfig::default(),
            tasks: TaskModelConfig::default(),
            models,
        }
    }
}

impl Default for ChatModelConfig {
    fn default() -> Self {
        Self {
            selected_model: "ollama-local".to_owned(),
        }
    }
}

impl Default for TaskModelConfig {
    fn default() -> Self {
        Self {
            default_model: "ollama-local".to_owned(),
        }
    }
}

impl Default for TaskConfig {
    fn default() -> Self {
        let directory = BaseDirs::new()
            .map(|base| base.config_dir().join("donna").join("tasks"))
            .unwrap_or_else(|| PathBuf::from("tasks"));

        Self { directory }
    }
}

impl Default for PromptsConfig {
    fn default() -> Self {
        let system_prompt_path = BaseDirs::new()
            .map(|base| {
                base.config_dir()
                    .join("donna")
                    .join("prompts")
                    .join("system.md")
            })
            .unwrap_or_else(|| PathBuf::from("prompts").join("system.md"));

        Self { system_prompt_path }
    }
}

impl Default for DataConfig {
    fn default() -> Self {
        let database_path = BaseDirs::new()
            .map(|base| base.data_dir().join("donna").join("donna.sqlite3"))
            .unwrap_or_else(|| PathBuf::from("donna.sqlite3"));

        Self {
            database_path,
            stale_after_minutes: 60,
        }
    }
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            require_sensitive_approval: true,
        }
    }
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            notification_min_level: "normal".to_owned(),
            popup_min_level: "important".to_owned(),
            popup_cooldown_seconds: 900,
            critical_bypasses_cooldown: true,
        }
    }
}

impl Default for OfflineConfig {
    fn default() -> Self {
        Self {
            show_stale_data_warnings: true,
            queue_external_actions: false,
        }
    }
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(formatter, "config IO error at {}: {source}", path.display())
            }
            ConfigError::Decode { path, source } => {
                write!(
                    formatter,
                    "invalid config TOML at {}: {source}",
                    path.display()
                )
            }
            ConfigError::Encode { source } => write!(formatter, "config encode error: {source}"),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ConfigError::Io { source, .. } => Some(source),
            ConfigError::Decode { source, .. } => Some(source),
            ConfigError::Encode { source } => Some(source),
        }
    }
}

fn default_models() -> Vec<ModelConfig> {
    vec![
        ModelConfig {
            id: "ollama-local".to_owned(),
            label: "Ollama local".to_owned(),
            provider: "ollama".to_owned(),
            model: "llama3.1".to_owned(),
            base_url: Some("http://localhost:11434".to_owned()),
            secret_ref: None,
        },
        ModelConfig {
            id: "openai-compatible".to_owned(),
            label: "OpenAI compatible".to_owned(),
            provider: "openai-compatible".to_owned(),
            model: "gpt-4.1-mini".to_owned(),
            base_url: Some("https://api.openai.com/v1".to_owned()),
            secret_ref: Some("donna/openai".to_owned()),
        },
        ModelConfig {
            id: "github-copilot-compatible".to_owned(),
            label: "GitHub Copilot compatible".to_owned(),
            provider: "github-copilot-compatible".to_owned(),
            model: "gpt-4.1".to_owned(),
            base_url: Some("https://api.githubcopilot.com".to_owned()),
            secret_ref: Some("donna/github-copilot".to_owned()),
        },
    ]
}

fn default_microsoft_scopes() -> Vec<String> {
    [
        "User.Read",
        "offline_access",
        "Mail.Read",
        "Mail.Send",
        "Calendars.ReadWrite",
        "ChatMessage.Send",
        "Chat.ReadWrite",
        "ChannelMessage.Read.All",
        "ChannelMessage.Send",
        "Team.ReadBasic.All",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[cfg(test)]
mod tests {
    use super::AppConfig;

    #[test]
    fn default_config_has_non_secret_model_references() {
        let config = AppConfig::default();

        assert_eq!(config.avatar.character, "donna");
        assert_eq!(config.ai.chat.selected_model, "ollama-local");
        assert!(
            config
                .ai
                .models
                .iter()
                .any(|model| model.secret_ref.is_none())
        );
        assert!(
            config
                .ai
                .models
                .iter()
                .any(|model| model.secret_ref.as_deref() == Some("donna/openai"))
        );
    }

    #[test]
    fn creates_and_reloads_default_config() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("donna.toml");

        let mut config = AppConfig::load_or_create_at(&path).expect("create config");
        config.ai.chat.selected_model = "openai-compatible".to_owned();
        config.data.database_path = dir.path().join("state.sqlite3");
        config.save_to_path(&path).expect("save config");

        let loaded = AppConfig::load_at(path).expect("reload config");
        assert_eq!(loaded.ai.chat.selected_model, "openai-compatible");
        assert_eq!(loaded.data.database_path, dir.path().join("state.sqlite3"));
        assert!(loaded.offline.show_stale_data_warnings);
    }

    #[test]
    fn invalid_config_falls_back_with_error_message() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("bad.toml");
        std::fs::write(&path, "[ai").expect("write bad toml");

        let (config, error) = AppConfig::load_or_default_at(path);
        assert_eq!(config.ai.chat.selected_model, "ollama-local");
        assert!(error.expect("error").contains("invalid config TOML"));
    }

    #[test]
    fn serialized_config_keeps_secret_values_out_of_toml() {
        let mut config = AppConfig::default();
        config.microsoft.client_id = Some("client-id".to_owned());
        config.microsoft.token_secret_ref = Some("donna/microsoft".to_owned());

        let contents = toml::to_string_pretty(&config).expect("serialize config");

        assert!(contents.contains("secret_ref = \"donna/openai\""));
        assert!(contents.contains("client_id = \"client-id\""));
        assert!(contents.contains("token_secret_ref = \"donna/microsoft\""));
        assert!(!contents.contains("api_key"));
        assert!(!contents.contains("access_token"));
        assert!(!contents.contains("refresh_token"));
        assert!(!contents.contains("password"));
    }
}
