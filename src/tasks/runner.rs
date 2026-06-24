use crate::ai::{AiError, ProviderCatalog};
use crate::model::ModelRegistry;
use crate::prompts::{LoadedPrompt, load_task_prompt};
use crate::tasks::{CronDateTime, TaskDefinition};
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunPlan {
    pub task_id: String,
    pub kind: String,
    pub model_id: String,
    pub prompt: LoadedPrompt,
    pub prompt_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskRunnerError {
    NoUsableTaskModel(String),
    Provider(AiError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunnerCore {
    default_model_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRunnerState {
    running: bool,
}

impl TaskRunnerCore {
    pub fn new(default_model_id: impl Into<String>) -> Self {
        Self {
            default_model_id: default_model_id.into(),
        }
    }

    pub fn due_task_plans(
        &self,
        registry: &ModelRegistry,
        tasks: &[TaskDefinition],
        at: CronDateTime,
    ) -> Result<Vec<TaskRunPlan>, TaskRunnerError> {
        let mut plans = Vec::new();

        for task in tasks {
            if !task.enabled || !task.schedule.matches(at) {
                continue;
            }

            let selection = ProviderCatalog::select_task_model(
                registry,
                &self.default_model_id,
                task.model_override.as_deref(),
            )
            .map_err(|error| match error {
                AiError::NoConfiguredModels => {
                    TaskRunnerError::NoUsableTaskModel(self.default_model_id.clone())
                }
                other => TaskRunnerError::Provider(other),
            })?;
            let prompt = load_task_prompt(task.kind.as_str(), task.prompt_file.as_deref());

            eprintln!(
                "{}",
                task_due_log_line(
                    &task.id,
                    task.kind.as_str(),
                    task.schedule.source(),
                    &selection.model.id,
                    at,
                )
            );

            plans.push(TaskRunPlan {
                task_id: task.id.clone(),
                kind: task.kind.as_str().to_owned(),
                model_id: selection.model.id,
                prompt,
                prompt_path: task
                    .prompt_file
                    .as_ref()
                    .map(|path| path.display().to_string()),
            });
        }

        Ok(plans)
    }
}

fn task_due_log_line(
    task_id: &str,
    kind: &str,
    cron: &str,
    model_id: &str,
    at: CronDateTime,
) -> String {
    format!(
        "donna task due: id={task_id} kind={kind} cron='{cron}' model={model_id} at={:02}:{:02} dom={} month={} dow={}",
        at.hour, at.minute, at.day_of_month, at.month, at.day_of_week
    )
}

impl TaskRunnerState {
    pub fn running() -> Self {
        Self { running: true }
    }

    pub fn stop(&mut self) {
        self.running = false;
    }

    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for TaskRunnerState {
    fn default() -> Self {
        Self::running()
    }
}

impl Display for TaskRunnerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskRunnerError::NoUsableTaskModel(model_id) => {
                write!(formatter, "no usable task model configured for {model_id}")
            }
            TaskRunnerError::Provider(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TaskRunnerError {}

#[cfg(test)]
mod tests {
    use super::{TaskRunnerCore, TaskRunnerState, task_due_log_line};
    use crate::config::AppConfig;
    use crate::model::ModelRegistry;
    use crate::tasks::{CronDateTime, TaskDefinition, TaskKind};

    #[test]
    fn plans_enabled_due_tasks_with_task_model() {
        let mut config = AppConfig::default();
        config.ai.chat.selected_model = "openai-compatible".to_owned();
        config.ai.tasks.default_model = "ollama-local".to_owned();
        let registry = ModelRegistry::from_config(&config);
        let task = TaskDefinition {
            id: "daily".to_owned(),
            enabled: true,
            kind: TaskKind::DailyPlanning,
            schedule: "0 8 * * *".parse().expect("cron"),
            prompt_file: None,
            model_override: None,
            config_path: "daily.toml".into(),
        };
        let runner = TaskRunnerCore::new(&config.ai.tasks.default_model);

        let plans = runner
            .due_task_plans(
                &registry,
                &[task],
                CronDateTime {
                    minute: 0,
                    hour: 8,
                    day_of_month: 10,
                    month: 6,
                    day_of_week: 2,
                },
            )
            .expect("plans");

        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].model_id, "ollama-local");
    }

    #[test]
    fn skips_disabled_tasks_and_non_matching_schedules() {
        let config = AppConfig::default();
        let registry = ModelRegistry::from_config(&config);
        let disabled = TaskDefinition {
            id: "disabled".to_owned(),
            enabled: false,
            kind: TaskKind::DailyPlanning,
            schedule: "0 8 * * *".parse().expect("cron"),
            prompt_file: None,
            model_override: None,
            config_path: "disabled.toml".into(),
        };
        let runner = TaskRunnerCore::new(&config.ai.tasks.default_model);

        let plans = runner
            .due_task_plans(
                &registry,
                &[disabled],
                CronDateTime {
                    minute: 0,
                    hour: 8,
                    day_of_month: 10,
                    month: 6,
                    day_of_week: 2,
                },
            )
            .expect("plans");

        assert!(plans.is_empty());
    }

    #[test]
    fn exit_can_stop_task_runner_state() {
        let mut state = TaskRunnerState::running();

        state.stop();

        assert!(!state.is_running());
    }

    #[test]
    fn due_task_log_line_includes_schedule_and_model() {
        let line = task_due_log_line(
            "daily",
            "daily_planning",
            "0 8 * * *",
            "ollama-local",
            CronDateTime {
                minute: 0,
                hour: 8,
                day_of_month: 10,
                month: 6,
                day_of_week: 2,
            },
        );

        assert!(line.contains("donna task due: id=daily"));
        assert!(line.contains("kind=daily_planning"));
        assert!(line.contains("cron='0 8 * * *'"));
        assert!(line.contains("model=ollama-local"));
        assert!(line.contains("at=08:00"));
    }
}
