use crate::approval::ApprovalError;
use crate::config::ConfigError;
use crate::secrets::SecretError;
use crate::storage::StorageError;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::SystemTimeError;

#[derive(Debug)]
pub enum GraphError {
    AdminConsentRequired { message: String },
    Approval(ApprovalError),
    Auth(String),
    Clock(SystemTimeError),
    Config(ConfigError),
    Http(reqwest::Error),
    Io(std::io::Error),
    Offline,
    Secret(SecretError),
    Storage(StorageError),
    TeamsPermissionUnavailable { message: String },
    UnexpectedResponse(String),
}

impl GraphError {
    pub fn auth_error(error: &str, description: Option<&str>) -> Self {
        let detail = description.unwrap_or(error).to_owned();
        let lower = detail.to_lowercase();

        if lower.contains("aadsts65001") || lower.contains("admin consent") {
            return Self::AdminConsentRequired { message: detail };
        }

        if lower.contains("teamwork") || lower.contains("channelmessage") {
            return Self::TeamsPermissionUnavailable { message: detail };
        }

        Self::Auth(detail)
    }

    pub fn is_permission_problem(&self) -> bool {
        matches!(
            self,
            Self::AdminConsentRequired { .. } | Self::TeamsPermissionUnavailable { .. }
        )
    }

    pub fn sync_error_message(&self) -> String {
        match self {
            Self::Secret(_) => {
                "Microsoft token could not be read from OS secret storage".to_owned()
            }
            _ => self.to_string(),
        }
    }
}

impl Display for GraphError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AdminConsentRequired { message } => write!(
                formatter,
                "Microsoft Graph needs tenant admin consent before Donna can continue: {message}"
            ),
            Self::Approval(source) => write!(formatter, "{source}"),
            Self::Auth(message) => write!(formatter, "Microsoft Graph auth failed: {message}"),
            Self::Clock(source) => write!(formatter, "system clock error: {source}"),
            Self::Config(source) => write!(formatter, "{source}"),
            Self::Http(source) => write!(formatter, "Microsoft Graph network error: {source}"),
            Self::Io(source) => write!(formatter, "auth wizard IO error: {source}"),
            Self::Offline => write!(
                formatter,
                "Donna is offline; Microsoft Graph actions and sync are paused"
            ),
            Self::Secret(source) => write!(formatter, "{source}"),
            Self::Storage(source) => write!(formatter, "{source}"),
            Self::TeamsPermissionUnavailable { message } => write!(
                formatter,
                "Microsoft Teams Graph permission is unavailable or not consented: {message}"
            ),
            Self::UnexpectedResponse(message) => {
                write!(formatter, "unexpected Microsoft Graph response: {message}")
            }
        }
    }
}

impl Error for GraphError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Approval(source) => Some(source),
            Self::Clock(source) => Some(source),
            Self::Config(source) => Some(source),
            Self::Http(source) => Some(source),
            Self::Io(source) => Some(source),
            Self::Secret(source) => Some(source),
            Self::Storage(source) => Some(source),
            Self::AdminConsentRequired { .. }
            | Self::Auth(_)
            | Self::Offline
            | Self::TeamsPermissionUnavailable { .. }
            | Self::UnexpectedResponse(_) => None,
        }
    }
}

impl From<ApprovalError> for GraphError {
    fn from(error: ApprovalError) -> Self {
        Self::Approval(error)
    }
}

impl From<ConfigError> for GraphError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<std::io::Error> for GraphError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SecretError> for GraphError {
    fn from(error: SecretError) -> Self {
        Self::Secret(error)
    }
}

impl From<StorageError> for GraphError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<SystemTimeError> for GraphError {
    fn from(error: SystemTimeError) -> Self {
        Self::Clock(error)
    }
}

impl From<reqwest::Error> for GraphError {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}
