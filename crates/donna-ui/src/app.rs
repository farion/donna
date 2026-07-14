use crate::avatar::AvatarManager;
use crate::ipc::{IpcEvent, wakeup_window};
use donna_ai::{
    AiMessage, AiProvider, AiRequest, AiRole, MockProvider, OllamaProvider,
    OpenAiCompatibleProvider, ProviderCatalog, ProviderFamily,
};
use donna_config::AppConfig;
use donna_core::chat::{ChatSession, Speaker};
use donna_core::command::{AppCommand, ParsedInput, parse_input};
use donna_core::model::ModelRegistry;
use donna_harness::memory::{MemoryExtractor, SensitiveMemoryApproval};
use donna_harness::prompts::load_system_prompt;
use donna_harness::tasks::{
    CronDateTime, TaskDefinition, TaskKind, TaskRunnerState, load_task_directory,
};
use donna_harness::tools::{
    execute_tool_call_from_model, humanize_model_todo_leak, local_tool_prompt,
};
use donna_integrations::microsoft::background_sync::run_sync_once as run_microsoft_sync_once;
use donna_integrations::secrets::{KeyringSecretStore, SecretStore};
use donna_storage::{LocalStore, StoredMemory};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Key, Layout, RichText, ScrollArea, Sense,
    UiBuilder, Vec2,
};
use egui_phosphor::Variant as PhosphorVariant;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod attention_ui;
mod chat_bar;
mod command_ui;
mod layout;
mod memory_review;
mod state;
mod status;
mod theme;
mod ui_style;

use attention_ui::AttentionUiState;
use chat_bar::chat_bar_reserved_height;
use layout::{
    CHAT_INNER_MARGIN, DEFAULT_WINDOW_SIZE, MIN_WINDOW_SIZE, avatar_image_size, shell_layout,
};
use state::{AvatarSignals, DonnaState, random_idle_frame, resolve_state};
use ui_style::{apply_style, palette_for, render_message};

const IDLE_DEFAULT_DURATION: Duration = Duration::from_secs(10);
const IDLE_PULSE_DURATION: Duration = Duration::from_millis(700);
const REMEMBERED_FACTS_CONTEXT_PROMPT: &str =
    include_str!("../../../assets/prompts/context/remembered_facts.md");
const CURRENT_OPEN_TODOS_CONTEXT_PROMPT: &str =
    include_str!("../../../assets/prompts/context/current_open_todos.md");

pub struct DonnaApp {
    config_path: PathBuf,
    config: AppConfig,
    config_notice: Option<String>,
    chat: ChatSession,
    store: Option<LocalStore>,
    memory_extractor: MemoryExtractor,
    sensitive_memory_reviews: memory_review::SensitiveMemoryReviews,
    task_runner_state: TaskRunnerState,
    task_definitions: Vec<TaskDefinition>,
    last_task_check_minute: Option<i64>,
    last_microsoft_sync_minute: Option<i64>,
    microsoft_sync_in_progress: bool,
    microsoft_sync_status: MicrosoftSyncUiStatus,
    microsoft_sync_receiver: Option<Receiver<Result<(), String>>>,
    input: String,
    input_history: Vec<String>,
    input_history_cursor: Option<usize>,
    input_history_draft: String,
    input_notice: Option<String>,
    input_error: Option<String>,
    pending_exit_confirmation: bool,
    models: ModelRegistry,
    selected_model_id: String,
    avatar_manager: AvatarManager,
    state: DonnaState,
    response_in_progress: bool,
    streaming_response: Option<StreamingResponse>,
    response_started_at: Option<Instant>,
    name_prompt_asked: bool,
    name_prompt_pending: bool,
    approval_pending: bool,
    attention: AttentionUiState,
    idle_frame: u8,
    last_idle_change: Instant,
    hide_requested: Arc<AtomicBool>,
    wakeup_receiver: Option<Arc<Mutex<Receiver<IpcEvent>>>>,
}

struct StreamingResponse {
    receiver: Receiver<ChatWorkerEvent>,
    message_id: u64,
    text: String,
    placeholder: String,
    user_message: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct MicrosoftSyncUiStatus {
    outlook_percent: u8,
    teams_percent: u8,
    calendar_percent: u8,
}

enum ChatWorkerEvent {
    Delta(String),
    Finished(String),
    Error(String),
}

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Donna")
            .with_app_id("donna")
            .with_inner_size(DEFAULT_WINDOW_SIZE)
            .with_min_inner_size(MIN_WINDOW_SIZE)
            .with_max_inner_size(DEFAULT_WINDOW_SIZE)
            .with_fullscreen(false)
            .with_maximized(false)
            .with_resizable(false)
            .with_transparent(true)
            .with_decorations(false)
            .with_maximize_button(false)
            .with_has_shadow(false)
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_buttons_shown(false)
            .with_titlebar_shown(false)
            .with_movable_by_background(true),
        ..Default::default()
    }
}

impl DonnaApp {
    pub fn new(creation: &eframe::CreationContext<'_>) -> Self {
        Self::new_with_config_path(creation, AppConfig::default_path())
    }

    pub fn new_with_hide_signal(
        creation: &eframe::CreationContext<'_>,
        hide_requested: Arc<AtomicBool>,
        wakeup_receiver: Arc<Mutex<Receiver<IpcEvent>>>,
    ) -> Self {
        Self::new_with_config_path_and_hide_signal(
            creation,
            AppConfig::default_path(),
            hide_requested,
            Some(wakeup_receiver),
        )
    }

    fn new_with_config_path(creation: &eframe::CreationContext<'_>, config_path: PathBuf) -> Self {
        Self::new_with_config_path_and_hide_signal(
            creation,
            config_path,
            Arc::new(AtomicBool::new(false)),
            None,
        )
    }

    fn new_with_config_path_and_hide_signal(
        creation: &eframe::CreationContext<'_>,
        config_path: PathBuf,
        hide_requested: Arc<AtomicBool>,
        wakeup_receiver: Option<Arc<Mutex<Receiver<IpcEvent>>>>,
    ) -> Self {
        let (mut config, config_notice) = AppConfig::load_or_default_at(&config_path);
        Self::install_phosphor_icons(&creation.egui_ctx);
        let (store, storage_notice) = match LocalStore::open(&config.data.database_path) {
            Ok(store) => (Some(store), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let mut config_notice = config_notice.or(storage_notice);
        let models = ModelRegistry::from_config(&config);
        let selected_model_id = models
            .normalized_selected_id(&config.ai.chat.selected_model)
            .unwrap_or_else(|| config.ai.chat.selected_model.clone());

        config.ai.chat.selected_model = selected_model_id.clone();
        apply_style(&creation.egui_ctx, config.ui.theme);
        let welcome_message = welcome_message_for_store(store.as_ref());
        let task_definitions = match load_task_directory(&config.tasks.directory) {
            Ok(tasks) => tasks,
            Err(error) => {
                config_notice = Some(error.to_string());
                Vec::new()
            }
        };
        let mut app = Self {
            config_path,
            memory_extractor: MemoryExtractor::from_config(&config.memory),
            config,
            config_notice,
            chat: ChatSession::with_welcome_message(welcome_message),
            store,
            task_runner_state: TaskRunnerState::running(),
            task_definitions,
            last_task_check_minute: None,
            last_microsoft_sync_minute: unix_now_seconds().map(|seconds| seconds / 60),
            microsoft_sync_in_progress: false,
            microsoft_sync_status: MicrosoftSyncUiStatus::default(),
            microsoft_sync_receiver: None,
            sensitive_memory_reviews: memory_review::SensitiveMemoryReviews::default(),
            input: String::new(),
            input_history: Vec::new(),
            input_history_cursor: None,
            input_history_draft: String::new(),
            input_notice: None,
            input_error: None,
            pending_exit_confirmation: false,
            models,
            selected_model_id,
            avatar_manager: AvatarManager::new(),
            state: DonnaState::Idle,
            response_in_progress: false,
            streaming_response: None,
            response_started_at: None,
            name_prompt_asked: false,
            name_prompt_pending: false,
            approval_pending: false,
            attention: AttentionUiState::default(),
            idle_frame: 0,
            last_idle_change: Instant::now(),
            hide_requested,
            wakeup_receiver,
        };
        app.trigger_microsoft_sync("startup");
        app
    }

    fn cycle_model(&mut self) {
        if let Some(next_model) = self.models.next_after(&self.selected_model_id) {
            self.selected_model_id = next_model.id.clone();
            self.config.ai.chat.selected_model = self.selected_model_id.clone();

            if let Err(error) = self.config.save_to_path(&self.config_path) {
                self.config_notice = Some(error.to_string());
            }
        }
    }

    fn submit_input(&mut self, ctx: &egui::Context) {
        let input = std::mem::take(&mut self.input);
        self.input_history_cursor = None;
        self.input_history_draft.clear();
        self.input_notice = None;
        self.input_error = None;

        match parse_input(&input) {
            ParsedInput::Empty => {}
            ParsedInput::Message(message) => {
                if self.response_in_progress {
                    self.input = message;
                    self.input_notice = Some("Donna is still thinking.".to_owned());
                    return;
                }
                self.remember_submitted_input(&input);
                self.pending_exit_confirmation = false;
                self.state = DonnaState::Idle;
                self.chat.push_user_message(message.as_str());
                self.persist_structured_chat_records(&message);
                if !self.name_prompt_asked && !self.knows_user_name() {
                    self.name_prompt_asked = true;
                    self.name_prompt_pending = true;
                }
                self.start_chat_response();
            }
            ParsedInput::Command(command) => {
                self.remember_submitted_input(&input);
                match command {
                    AppCommand::Exit { confirmed: _ } => self.handle_exit_command(ctx),
                    AppCommand::Hide => self.handle_hide_command(ctx),
                    AppCommand::ChangeCharacter(character) => {
                        self.handle_change_character_command(character.as_deref());
                    }
                    AppCommand::Theme(theme) => {
                        self.handle_theme_command(theme.as_deref(), ctx);
                    }
                    AppCommand::Task(task_name) => {
                        self.handle_task_command(task_name.as_deref());
                    }
                    AppCommand::Forget => self.handle_forget_command(),
                    AppCommand::Unknown(command) => {
                        self.show_command_error(format!("Unknown command: /{command}"))
                    }
                }
            }
        }
    }

    fn remember_submitted_input(&mut self, input: &str) {
        if input.trim().is_empty() {
            return;
        }
        self.input_history.push(input.to_owned());
    }

    fn handle_exit_command(&mut self, ctx: &egui::Context) {
        self.state = DonnaState::Command;
        self.pending_exit_confirmation = false;
        self.hide_requested.store(false, Ordering::SeqCst);
        self.task_runner_state.stop();
        self.input_notice = Some("Stopping Donna.".to_owned());
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn handle_hide_command(&mut self, ctx: &egui::Context) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Hidden;
        self.input_notice = Some("Donna is hidden. Background tasks keep running.".to_owned());
        self.hide_requested.store(false, Ordering::SeqCst);
        if !hide_with_compositor() {
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
        }
    }

    fn handle_change_character_command(&mut self, character: Option<&str>) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Command;

        let Some(character) = character else {
            self.show_command_error("Usage: /changechar [name]");
            return;
        };

        if !AvatarManager::character_exists(character) {
            self.show_command_error(format!("Unknown avatar character: {character}"));
            return;
        }

        self.config.avatar.character = character.to_owned();
        match self.config.save_to_path(&self.config_path) {
            Ok(()) => {
                self.input_notice = Some(format!("Avatar changed to {character}."));
            }
            Err(error) => self.config_notice = Some(error.to_string()),
        }
    }

    fn handle_task_command(&mut self, task_name: Option<&str>) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Command;
        let Some(task_name) = task_name.filter(|name| !name.trim().is_empty()) else {
            self.show_command_error("Usage: /task [name]");
            return;
        };
        let normalized_name = normalize_task_name(task_name);
        let Some(task) = self
            .task_definitions
            .iter()
            .find(|task| {
                normalize_task_name(&task.id) == normalized_name
                    || normalize_task_name(task.kind.as_str()) == normalized_name
            })
            .cloned()
        else {
            self.show_command_error(format!("Unknown task: {task_name}"));
            return;
        };
        if !task.enabled {
            self.show_command_error(format!("Task is disabled: {}", task.id));
            return;
        }

        let now = unix_now_seconds().unwrap_or(0);
        match self.execute_local_task(&task, now, cron_datetime_from_unix(now), true) {
            LocalTaskOutcome::CreatedAttention(id) => {
                self.input_notice = Some(format!("Task {} ran. Reminder #{id} created.", task.id));
            }
            LocalTaskOutcome::Noop => {
                self.input_notice = Some(format!("Task {} ran. Nothing to remind.", task.id));
            }
            LocalTaskOutcome::Unsupported => {
                self.show_command_error(format!("Task cannot be run locally yet: {}", task.id));
            }
            LocalTaskOutcome::Error(error) => self.show_command_error(error),
        }
    }

    fn handle_forget_command(&mut self) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Command;
        let Some(store) = &self.store else {
            self.show_command_error("Storage unavailable; cannot forget task snoozes.");
            return;
        };

        match store.forget_task_reminder_snoozes() {
            Ok(0) => self.input_notice = Some("No task snoozes to forget.".to_owned()),
            Ok(count) => {
                self.input_notice = Some(format!("Forgot {count} task snooze records."));
            }
            Err(error) => self.show_command_error(error.to_string()),
        }
    }

    fn show_command_error(&mut self, error: impl Into<String>) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Command;
        self.input_error = Some(error.into());
    }

    fn refresh_idle_frame(&mut self, ctx: &egui::Context) {
        if self.visual_state() != DonnaState::Idle {
            return;
        }

        let elapsed = self.last_idle_change.elapsed();
        if self.idle_frame == 0 && elapsed >= IDLE_DEFAULT_DURATION {
            self.idle_frame = random_idle_frame();
            self.last_idle_change = Instant::now();
        } else if self.idle_frame != 0 && elapsed >= IDLE_PULSE_DURATION {
            self.idle_frame = 0;
            self.last_idle_change = Instant::now();
        }

        let repaint_after = if self.idle_frame == 0 {
            IDLE_DEFAULT_DURATION.saturating_sub(self.last_idle_change.elapsed())
        } else {
            IDLE_PULSE_DURATION.saturating_sub(self.last_idle_change.elapsed())
        };
        ctx.request_repaint_after(repaint_after.max(Duration::from_millis(16)));
    }

    fn poll_wakeup_ipc(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.wakeup_receiver else {
            return;
        };
        let Ok(receiver) = receiver.lock() else {
            return;
        };

        let mut woke = false;
        while matches!(receiver.try_recv(), Ok(IpcEvent::Wakeup)) {
            woke = true;
        }
        if woke {
            self.state = DonnaState::Idle;
            self.hide_requested.store(false, Ordering::SeqCst);
            wakeup_window();
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn avatar_state(&self) -> crate::avatar::AvatarState {
        self.visual_state().avatar_state(self.idle_frame)
    }

    fn state_label(&self) -> String {
        if self.visual_state() == DonnaState::Thinking {
            let dots = self
                .response_started_at
                .map(|started| (started.elapsed().as_millis() / 350 % 3) + 1)
                .unwrap_or(3);
            return format!("thinking{}", ".".repeat(dots as usize));
        }

        self.visual_state().label().to_owned()
    }

    fn visual_state(&self) -> DonnaState {
        resolve_state(AvatarSignals {
            command_mode: self.input.trim_start().starts_with('/')
                || self.pending_exit_confirmation
                || self.state == DonnaState::Command,
            hidden: self.state == DonnaState::Hidden,
            active_response: self.response_in_progress,
            active_question: self.approval_pending,
            active_attention: self.attention.has_active_item(),
        })
    }

    fn persist_structured_chat_records(&mut self, message: &str) -> String {
        let extraction = self.memory_extractor.extract_user_message(message);
        if extraction.memories.is_empty()
            && extraction.sensitive_memories.is_empty()
            && extraction.todos.is_empty()
            && extraction.people.is_empty()
        {
            return "I kept this exchange in memory only.".to_owned();
        }

        let Some(store) = &self.store else {
            return "I identified structured information, but local storage is unavailable."
                .to_owned();
        };

        let sensitive_count = extraction.sensitive_memories.len();
        match self.memory_extractor.persist(
            store,
            &extraction,
            SensitiveMemoryApproval::RejectSensitive,
        ) {
            Ok(persisted) if persisted.has_records() => {
                let mut note = format!(
                    "I saved {} structured item(s) and did not store the raw chat.",
                    persisted.record_count()
                );
                if persisted.skipped_sensitive > 0 {
                    self.sensitive_memory_reviews
                        .queue(extraction.sensitive_memories.clone());
                    note.push_str(&format!(
                        " {} sensitive memory item(s) need review before saving.",
                        sensitive_count
                    ));
                }
                note
            }
            Ok(persisted) if persisted.skipped_sensitive > 0 => {
                self.sensitive_memory_reviews
                    .queue(extraction.sensitive_memories.clone());
                format!(
                    "{} sensitive memory item(s) need review before saving. Nothing sensitive was saved yet.",
                    sensitive_count
                )
            }
            Ok(_) => "I kept this exchange in memory only.".to_owned(),
            Err(error) => {
                self.config_notice = Some(error.to_string());
                "I could not save structured records because local storage returned an error."
                    .to_owned()
            }
        }
    }

    fn start_chat_response(&mut self) {
        let selection =
            match ProviderCatalog::select_chat_model(&self.models, &self.selected_model_id) {
                Ok(selection) => selection,
                Err(error) => {
                    self.chat
                        .push_donna_message(format!("I could not select the chat model: {error}"));
                    return;
                }
            };
        let prompt = load_system_prompt(&self.config);
        if let Some(notice) = prompt.notice {
            self.config_notice = Some(notice);
        }
        let system_prompt = self.system_prompt_with_memories(prompt.content);
        let request =
            self.chat
                .messages()
                .iter()
                .fold(
                    AiRequest::new(system_prompt),
                    |request, message| match message.speaker {
                        Speaker::Donna => request.with_message(AiMessage::trusted(
                            AiRole::Assistant,
                            message.text.as_str(),
                        )),
                        Speaker::User => request
                            .with_message(AiMessage::trusted(AiRole::User, message.text.as_str())),
                    },
                );
        let auth_material = match load_model_auth_material(&selection) {
            Ok(auth_material) => auth_material,
            Err(error) => {
                self.chat.push_donna_message(error);
                return;
            }
        };
        let user_message = self
            .chat
            .messages()
            .iter()
            .rev()
            .find(|message| message.speaker == Speaker::User)
            .map(|message| message.text.clone())
            .unwrap_or_default();

        let Some(message_id) = self.chat.push_donna_message("...") else {
            return;
        };
        let (sender, receiver) = mpsc::channel();
        self.streaming_response = Some(StreamingResponse {
            receiver,
            message_id,
            text: String::new(),
            placeholder: String::new(),
            user_message,
        });
        self.response_in_progress = true;
        self.response_started_at = Some(Instant::now());
        thread::spawn(move || run_chat_worker(selection, request, auth_material, sender));
    }

    fn system_prompt_with_memories(&mut self, mut system_prompt: String) -> String {
        let Some(store) = &self.store else {
            return system_prompt;
        };
        let memories = match store.recent_memories(40) {
            Ok(memories) => memories,
            Err(error) => {
                self.config_notice = Some(error.to_string());
                return system_prompt;
            }
        };
        if !memories.is_empty() {
            system_prompt.push_str(REMEMBERED_FACTS_CONTEXT_PROMPT);
            for memory in memories {
                system_prompt.push_str("- ");
                system_prompt.push_str(&memory.content);
                system_prompt.push('\n');
            }
        }
        self.append_open_todos_to_prompt(&mut system_prompt);
        system_prompt
    }

    fn append_open_todos_to_prompt(&mut self, system_prompt: &mut String) {
        let Some(store) = &self.store else {
            return;
        };
        let todos = match store.open_todos(40) {
            Ok(todos) => todos,
            Err(error) => {
                self.config_notice = Some(error.to_string());
                return;
            }
        };

        system_prompt.push_str(CURRENT_OPEN_TODOS_CONTEXT_PROMPT);
        if todos.is_empty() {
            system_prompt.push_str("None.\n");
        } else {
            for todo in todos {
                system_prompt.push_str("- id=");
                system_prompt.push_str(&todo.id.to_string());
                system_prompt.push_str(" severity=");
                system_prompt.push_str(&todo.severity);
                if let Some(due_at) = todo.due_at {
                    system_prompt.push_str(" due_at=");
                    system_prompt.push_str(&due_at.to_string());
                }
                system_prompt.push_str(" title=");
                system_prompt.push_str(&todo.title);
                system_prompt.push('\n');
            }
        }

        system_prompt.push_str(&local_tool_prompt());
    }

    fn poll_chat_worker(&mut self, ctx: &egui::Context) {
        let Some(streaming) = &mut self.streaming_response else {
            return;
        };
        let mut finished = false;
        while let Ok(event) = streaming.receiver.try_recv() {
            match event {
                ChatWorkerEvent::Delta(delta) => {
                    streaming.text.push_str(&delta);
                    self.chat
                        .replace_message_text(streaming.message_id, streaming.text.clone());
                }
                ChatWorkerEvent::Finished(text) => {
                    if let Some(tool_result) = execute_tool_call_from_model(
                        self.store.as_ref(),
                        &text,
                        &streaming.user_message,
                    ) {
                        self.chat
                            .replace_message_text(streaming.message_id, tool_result);
                        finished = true;
                        continue;
                    }
                    if streaming.text.is_empty() {
                        let text = if text.trim().is_empty() {
                            "The selected model returned an empty response.".to_owned()
                        } else {
                            humanize_model_todo_leak(self.store.as_ref(), text)
                        };
                        self.chat.replace_message_text(streaming.message_id, text);
                    }
                    finished = true;
                }
                ChatWorkerEvent::Error(error) => {
                    self.chat.replace_message_text(streaming.message_id, error);
                    finished = true;
                }
            }
        }

        if finished {
            self.streaming_response = None;
            self.response_in_progress = false;
            self.response_started_at = None;
            if self.name_prompt_pending {
                self.name_prompt_pending = false;
                self.chat.push_donna_message(
                    "By the way, what should I call you? I like knowing whose chaos I'm taming.",
                );
            }
        } else {
            if streaming.text.is_empty() {
                let dots = self
                    .response_started_at
                    .map(|started| (started.elapsed().as_millis() / 350 % 3) + 1)
                    .unwrap_or(3);
                let placeholder = ".".repeat(dots as usize);
                if streaming.placeholder != placeholder {
                    streaming.placeholder = placeholder.clone();
                    self.chat
                        .replace_message_text(streaming.message_id, placeholder);
                }
            }
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn knows_user_name(&mut self) -> bool {
        let Some(store) = &self.store else {
            return false;
        };
        match store.recent_memories(40) {
            Ok(memories) => memories
                .iter()
                .any(|memory| memory.content.starts_with("User name: ")),
            Err(error) => {
                self.config_notice = Some(error.to_string());
                false
            }
        }
    }

    fn run_due_local_tasks(&mut self) {
        if !self.task_runner_state.is_running() {
            return;
        }
        let Some(now) = unix_now_seconds() else {
            return;
        };
        let minute_key = now / 60;
        if self.last_task_check_minute == Some(minute_key) {
            return;
        }
        self.last_task_check_minute = Some(minute_key);
        self.run_due_local_tasks_at(now, cron_datetime_from_unix(now));
    }

    fn run_due_microsoft_sync(&mut self) {
        if self.store.is_none() {
            return;
        }
        let Some(now) = unix_now_seconds() else {
            return;
        };
        let minute_key = now / 60;
        if self.last_microsoft_sync_minute == Some(minute_key) {
            return;
        }
        self.last_microsoft_sync_minute = Some(minute_key);
        self.trigger_microsoft_sync(&format!("minute={minute_key}"));
    }

    fn trigger_microsoft_sync(&mut self, reason: &str) {
        if self.microsoft_sync_in_progress {
            eprintln!("donna ui: Microsoft sync already running, skip trigger ({reason})");
            return;
        }
        eprintln!("donna ui: triggering Microsoft sync ({reason})");
        let config = self.config.clone();
        let database_path = config.data.database_path.clone();
        let (sender, receiver) = mpsc::channel();
        self.microsoft_sync_receiver = Some(receiver);
        self.microsoft_sync_in_progress = true;
        thread::spawn(move || {
            let result = LocalStore::open(&database_path)
                .map_err(|error| format!("storage unavailable: {error}"))
                .and_then(|store| {
                    run_microsoft_sync_once(&store, &config, &KeyringSecretStore::default())
                        .map_err(|error| error.to_string())
                });
            let _ = sender.send(result);
        });
    }

    fn poll_microsoft_sync_worker(&mut self) {
        self.refresh_microsoft_sync_status();
        let Some(receiver) = self.microsoft_sync_receiver.as_ref() else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("Microsoft sync worker disconnected".to_owned()))
            }
        };

        if let Some(result) = result {
            self.microsoft_sync_in_progress = false;
            self.microsoft_sync_receiver = None;
            self.refresh_microsoft_sync_status();
            match result {
                Ok(()) => eprintln!("donna ui: Microsoft sync worker finished"),
                Err(error) => {
                    eprintln!("donna ui: Microsoft sync worker failed: {error}");
                    self.config_notice = Some(error);
                }
            }
        }
    }

    fn refresh_microsoft_sync_status(&mut self) {
        let Some(store) = self.store.as_ref() else {
            self.microsoft_sync_status = MicrosoftSyncUiStatus::default();
            return;
        };

        self.microsoft_sync_status = MicrosoftSyncUiStatus {
            outlook_percent: Self::runtime_percent(store, "microsoft.sync.progress.outlook"),
            teams_percent: Self::runtime_percent(store, "microsoft.sync.progress.teams"),
            calendar_percent: Self::runtime_percent(store, "microsoft.sync.progress.calendar"),
        };
    }

    fn runtime_percent(store: &LocalStore, key: &str) -> u8 {
        store
            .runtime_state(key)
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u8>().ok())
            .unwrap_or(0)
    }

    fn install_phosphor_icons(ctx: &egui::Context) {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, PhosphorVariant::Regular);
        ctx.set_fonts(fonts);
    }

    fn run_due_local_tasks_at(&mut self, now: i64, at: CronDateTime) {
        for task in self.task_definitions.clone() {
            if !task.enabled || !task.schedule.matches(at) {
                continue;
            }
            if let LocalTaskOutcome::Error(error) = self.execute_local_task(&task, now, at, false) {
                self.config_notice = Some(error);
            }
        }
    }

    fn execute_local_task(
        &mut self,
        task: &TaskDefinition,
        now: i64,
        at: CronDateTime,
        forced: bool,
    ) -> LocalTaskOutcome {
        let Some(store) = &self.store else {
            return LocalTaskOutcome::Error("Storage unavailable; cannot run task.".to_owned());
        };
        if task.kind != TaskKind::TodoReminder {
            return LocalTaskOutcome::Unsupported;
        }

        match store.create_todo_reminder_attention(now) {
            Ok(Some(item)) => {
                eprintln!(
                    "donna task executed: id={} kind={} attention_item={} at={:02}:{:02}{}",
                    task.id,
                    task.kind.as_str(),
                    item.id,
                    at.hour,
                    at.minute,
                    if forced { " forced=true" } else { "" }
                );
                LocalTaskOutcome::CreatedAttention(item.id)
            }
            Ok(None) => {
                if forced {
                    eprintln!(
                        "donna task executed: id={} kind={} result=noop forced=true",
                        task.id,
                        task.kind.as_str()
                    );
                }
                LocalTaskOutcome::Noop
            }
            Err(error) => LocalTaskOutcome::Error(error.to_string()),
        }
    }
}

enum LocalTaskOutcome {
    CreatedAttention(i64),
    Noop,
    Unsupported,
    Error(String),
}

fn welcome_message_for_store(store: Option<&LocalStore>) -> String {
    let Some(memories) = store
        .and_then(|store| store.recent_memories(40).ok())
        .filter(|memories| !memories.is_empty())
    else {
        return fallback_welcome_message();
    };

    personalized_welcome(&memories).unwrap_or_else(fallback_welcome_message)
}

fn personalized_welcome(memories: &[StoredMemory]) -> Option<String> {
    let name = memory_value(memories, "User name: ");
    let role = memory_value(memories, "Fact: User role: ");
    let workplace = memory_value(memories, "Fact: User workplace: ");
    let place = memory_value(memories, "Fact: User lives in: ");

    if let Some(name) = name {
        return Some(match (role, workplace, place) {
            (Some(role), Some(workplace), _) => {
                format!("{name}, my favorite {role} from {workplace}. What are we taming today?")
            }
            (Some(role), _, _) => format!("{name}, my favorite {role}. What are we taming today?"),
            (_, Some(workplace), _) => {
                format!("{name} from {workplace}. What needs handling?")
            }
            (_, _, Some(place)) => format!("{name}, trouble from {place}. What needs handling?"),
            _ => format!("{name}. I remembered. What shall we make behave?"),
        });
    }

    if let Some(workplace) = workplace {
        return Some(format!(
            "Back from {workplace}, are we? Give me the first target."
        ));
    }

    if let Some(role) = role {
        return Some(format!("My sharp {role} is back. What are we handling?"));
    }

    place.map(|place| format!("Back from {place}, are we? Give me the first target."))
}

fn memory_value<'a>(memories: &'a [StoredMemory], prefix: &str) -> Option<&'a str> {
    memories
        .iter()
        .find_map(|memory| memory.content.strip_prefix(prefix))
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn fallback_welcome_message() -> String {
    ChatSession::with_welcome()
        .messages()
        .first()
        .map(|message| message.text.clone())
        .unwrap_or_else(|| "Donna is ready.".to_owned())
}

fn run_chat_worker(
    selection: donna_ai::ProviderSelection,
    request: AiRequest,
    auth_material: Option<String>,
    sender: Sender<ChatWorkerEvent>,
) {
    let result = match selection.family {
        ProviderFamily::Ollama => {
            OllamaProvider.complete_streaming(&selection.model, &request, |delta| {
                let _ = sender.send(ChatWorkerEvent::Delta(delta.to_owned()));
            })
        }
        ProviderFamily::Mock => {
            let response = selection
                .model
                .model
                .strip_prefix("mock-response:")
                .unwrap_or("Mock response");
            MockProvider::new(response).complete(&selection.model, &request)
        }
        ProviderFamily::OpenAiCompatible | ProviderFamily::GithubCopilotCompatible => {
            let Some(auth_material) = auth_material else {
                let _ = sender.send(ChatWorkerEvent::Error(format!(
                    "{} is selected, but its auth secret is missing. Run donna --auth.",
                    selection.model.label
                )));
                return;
            };
            OpenAiCompatibleProvider::new(selection.family, auth_material)
                .complete(&selection.model, &request)
        }
    };

    match result {
        Ok(response) => {
            let _ = sender.send(ChatWorkerEvent::Finished(response.text));
        }
        Err(error) => {
            let _ = sender.send(ChatWorkerEvent::Error(format!(
                "{} could not answer: {error}",
                selection.model.label
            )));
        }
    }
}

fn load_model_auth_material(
    selection: &donna_ai::ProviderSelection,
) -> Result<Option<String>, String> {
    if !selection.capabilities.requires_secret {
        return Ok(None);
    }
    let Some(secret_ref) = selection.model.secret_ref.as_deref() else {
        return Err(format!(
            "{} needs auth, but no secret reference is configured.",
            selection.model.label
        ));
    };
    let store = KeyringSecretStore::default();
    let secret = store
        .get_secret(secret_ref)
        .map_err(|error| format!("Could not read {secret_ref} from OS secret storage: {error}"))?;
    secret
        .filter(|value| !value.trim().is_empty())
        .map(Some)
        .ok_or_else(|| {
            format!(
                "{} auth is missing at secret reference '{secret_ref}'. Run donna --auth again to refresh OS secret storage.",
                selection.model.label
            )
        })
}

impl eframe::App for DonnaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_wakeup_ipc(ctx);
        self.poll_chat_worker(ctx);
        self.poll_microsoft_sync_worker();
        self.run_due_microsoft_sync();
        self.run_due_local_tasks();
        if self.input.trim_start().starts_with('/')
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter))
        {
            self.submit_input(ctx);
            return;
        }
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Tab)) {
            self.cycle_model();
        }

        self.refresh_idle_frame(ctx);
        self.attention.refresh(
            self.store.as_ref(),
            &self.config.attention,
            ctx,
            &mut self.config_notice,
        );
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.state == DonnaState::Hidden && self.visual_state() == DonnaState::Hidden {
            return;
        }

        let ctx = ui.ctx().clone();
        let available = ui.available_size();
        let layout = shell_layout(available);
        let content_width = layout.avatar_width + layout.gap + layout.chat_width;
        let content_height = layout.avatar_height.max(layout.chat_height);
        let left_space = ((available.x - content_width) / 2.0).max(0.0);
        let top_space = ((available.y - content_height) / 2.0).max(0.0);

        ui.add_space(top_space);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.add_space(left_space);
            self.render_avatar(ui, Vec2::new(layout.avatar_width, layout.avatar_height));
            ui.add_space(layout.gap);
            self.render_chat(ui, Vec2::new(layout.chat_width, layout.chat_height), &ctx);
        });
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }
}

fn unix_now_seconds() -> Option<i64> {
    let seconds = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    i64::try_from(seconds).ok()
}

fn cron_datetime_from_unix(seconds: i64) -> CronDateTime {
    let days = seconds.div_euclid(86_400);
    let seconds_of_day = seconds.rem_euclid(86_400);
    let (_, month, day) = civil_from_days(days);
    CronDateTime {
        minute: ((seconds_of_day / 60) % 60) as u8,
        hour: (seconds_of_day / 3_600) as u8,
        day_of_month: day,
        month,
        day_of_week: ((days + 4).rem_euclid(7)) as u8,
    }
}

fn hide_with_compositor() -> bool {
    if std::env::var_os("SWAYSOCK").is_some() {
        return command_succeeds("swaymsg", &[r#"[app_id="donna"] move scratchpad"#]);
    }
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return command_succeeds(
            "hyprctl",
            &["dispatch", "movetoworkspacesilent", "special:donna"],
        );
    }

    false
}

fn command_succeeds(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn normalize_task_name(name: &str) -> String {
    name.trim().to_ascii_lowercase().replace('-', "_")
}

fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u8, day as u8)
}

impl DonnaApp {
    fn render_avatar(&mut self, ui: &mut egui::Ui, size: Vec2) {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let character = self.config.avatar.character.as_str();

        if let Some(texture) =
            self.avatar_manager
                .texture_for(ui.ctx(), character, self.avatar_state())
        {
            let image_size = avatar_image_size(texture.size_vec2(), size);
            if image_size.x > 0.0 && image_size.y > 0.0 {
                let image_rect = egui::Rect::from_center_size(rect.center(), image_size);
                ui.painter().image(
                    texture.id(),
                    image_rect,
                    egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
    }

    fn render_chat(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        let palette = palette_for(ui.ctx().theme());
        let margin = CHAT_INNER_MARGIN;
        let inner_size = Vec2::new(
            (size.x - margin * 2.0).max(0.0),
            (size.y - margin * 2.0).max(0.0),
        );
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());

        ui.painter()
            .rect_filled(rect, CornerRadius::same(8), palette.chat_fill);

        if inner_size.x <= 0.0 || inner_size.y <= 0.0 {
            return;
        }

        let inner_rect = rect.shrink(margin);
        let mut chat_ui = ui.new_child(
            UiBuilder::new()
                .id_salt("chat-panel")
                .max_rect(inner_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        chat_ui.set_clip_rect(inner_rect);
        chat_ui.set_width(inner_size.x);
        chat_ui.set_height(inner_size.y);

        let activity_rect = egui::Rect::from_min_size(
            egui::pos2((inner_rect.right() - 84.0).max(inner_rect.left()), inner_rect.top()),
            Vec2::new(84.0, 18.0),
        );
        let mut activity_ui = ui.new_child(
            UiBuilder::new()
                .id_salt("chat-activity-strip")
                .max_rect(activity_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        activity_ui.set_clip_rect(inner_rect);
        self.render_activity_strip(&mut activity_ui, false);

        let input_height = chat_bar_reserved_height(inner_size.x, &self.input, Some(ctx));
        let attention_height = if self.attention.has_active_item() {
            let before = chat_ui.cursor().top();
            self.attention
                .render(&mut chat_ui, self.store.as_ref(), &mut self.config_notice);
            chat_ui.add_space(8.0);
            (chat_ui.cursor().top() - before).max(0.0)
        } else {
            0.0
        };
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height((inner_size.y - input_height - attention_height).max(24.0))
            .show(&mut chat_ui, |ui| {
                ui.set_width(inner_size.x);
                for message in self.chat.messages() {
                    render_message(
                        ui,
                        message.speaker,
                        &message.text,
                        inner_size.x,
                        &self.config,
                    );
                    ui.add_space(8.0);
                }

                if let Some(notice) = &self.config_notice {
                    ui.label(
                        RichText::new(notice)
                            .font(FontId::proportional(12.0))
                            .color(palette.warning_text),
                    );
                }

                self.sensitive_memory_reviews.render(
                    ui,
                    self.store.as_ref(),
                    &mut self.config_notice,
                    inner_size.x,
                );
            });

        chat_ui.separator();
        chat_ui.allocate_ui_with_layout(
            Vec2::new(inner_size.x, input_height),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(inner_size.x);
                self.render_chat_bar(ui, ctx);
            },
        );
    }
}

#[cfg(test)]
mod tests;
