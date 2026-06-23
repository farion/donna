use crate::tasks::CronSchedule;
use serde::Deserialize;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskDefinition {
    pub id: String,
    pub enabled: bool,
    pub kind: TaskKind,
    pub schedule: CronSchedule,
    pub prompt_file: Option<PathBuf>,
    pub model_override: Option<String>,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskKind {
    DailyPlanning,
    ShutdownReview,
    CalendarCollision,
    MailFollowUp,
    Generic(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskConfigError {
    Io { path: PathBuf, message: String },
    Decode { path: PathBuf, message: String },
    Invalid { path: PathBuf, message: String },
}

#[derive(Debug, Deserialize)]
struct RawTaskDefinition {
    id: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
    kind: String,
    cron: String,
    prompt_file: Option<PathBuf>,
    model: Option<String>,
}

pub fn load_task_directory(
    directory: impl AsRef<Path>,
) -> Result<Vec<TaskDefinition>, TaskConfigError> {
    let directory = directory.as_ref();
    if !directory.exists() {
        return Ok(Vec::new());
    }

    let mut paths = fs::read_dir(directory)
        .map_err(|error| TaskConfigError::Io {
            path: directory.to_owned(),
            message: error.to_string(),
        })?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("toml"))
        .collect::<Vec<_>>();
    paths.sort();

    paths.into_iter().map(load_task_file).collect()
}

fn load_task_file(path: PathBuf) -> Result<TaskDefinition, TaskConfigError> {
    let contents = fs::read_to_string(&path).map_err(|error| TaskConfigError::Io {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let raw = toml::from_str::<RawTaskDefinition>(&contents).map_err(|error| {
        TaskConfigError::Decode {
            path: path.clone(),
            message: error.to_string(),
        }
    })?;

    if raw.id.trim().is_empty() {
        return Err(TaskConfigError::Invalid {
            path,
            message: "task id cannot be empty".to_owned(),
        });
    }

    let schedule = CronSchedule::from_str(&raw.cron).map_err(|error| TaskConfigError::Invalid {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let prompt_file = raw
        .prompt_file
        .map(|prompt| resolve_prompt_path(&path, prompt));

    Ok(TaskDefinition {
        id: raw.id,
        enabled: raw.enabled,
        kind: TaskKind::from_name(&raw.kind),
        schedule,
        prompt_file,
        model_override: raw.model.filter(|model| !model.trim().is_empty()),
        config_path: path,
    })
}

fn resolve_prompt_path(config_path: &Path, prompt: PathBuf) -> PathBuf {
    if prompt.is_absolute() {
        return prompt;
    }

    config_path
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(prompt)
}

fn default_enabled() -> bool {
    true
}

impl TaskKind {
    pub fn from_name(value: &str) -> Self {
        match value {
            "daily_planning" => Self::DailyPlanning,
            "shutdown_review" => Self::ShutdownReview,
            "calendar_collision" => Self::CalendarCollision,
            "mail_follow_up" => Self::MailFollowUp,
            other => Self::Generic(other.to_owned()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::DailyPlanning => "daily_planning",
            Self::ShutdownReview => "shutdown_review",
            Self::CalendarCollision => "calendar_collision",
            Self::MailFollowUp => "mail_follow_up",
            Self::Generic(kind) => kind.as_str(),
        }
    }
}

impl Display for TaskConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskConfigError::Io { path, message } => {
                write!(
                    formatter,
                    "task config IO error at {}: {message}",
                    path.display()
                )
            }
            TaskConfigError::Decode { path, message } => {
                write!(
                    formatter,
                    "invalid task TOML at {}: {message}",
                    path.display()
                )
            }
            TaskConfigError::Invalid { path, message } => {
                write!(
                    formatter,
                    "invalid task config at {}: {message}",
                    path.display()
                )
            }
        }
    }
}

impl Error for TaskConfigError {}

#[cfg(test)]
mod tests {
    use super::{TaskKind, load_task_directory};

    #[test]
    fn loads_task_toml_with_markdown_prompt_reference() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("daily.toml"),
            r#"
id = "daily"
enabled = true
kind = "daily_planning"
cron = "0 8 * * 1-5"
prompt_file = "daily.md"
model = "openai-compatible"
"#,
        )
        .expect("write task");
        std::fs::write(dir.path().join("daily.md"), "# Daily\n").expect("write prompt");

        let tasks = load_task_directory(dir.path()).expect("tasks");

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, "daily");
        assert_eq!(tasks[0].kind, TaskKind::DailyPlanning);
        assert_eq!(tasks[0].prompt_file, Some(dir.path().join("daily.md")));
        assert_eq!(
            tasks[0].model_override.as_deref(),
            Some("openai-compatible")
        );
    }

    #[test]
    fn reports_invalid_task_config_clearly() {
        let dir = tempfile::tempdir().expect("dir");
        std::fs::write(
            dir.path().join("bad.toml"),
            r#"
id = "bad"
kind = "daily_planning"
cron = "not cron"
"#,
        )
        .expect("write task");

        let error = load_task_directory(dir.path()).expect_err("invalid");

        assert!(error.to_string().contains("invalid task config"));
        assert!(error.to_string().contains("bad.toml"));
    }
}
