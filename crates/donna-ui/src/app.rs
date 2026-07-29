use crate::avatar::AvatarManager;
use crate::ipc::{IpcEvent, RepaintSignal, new_repaint_signal, set_repaint_context};
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
    tag_calendar_followup_with_event_id,
};
use donna_integrations::microsoft::background_sync::run_sync_once as run_microsoft_sync_once;
use donna_integrations::secrets::{KeyringSecretStore, SecretStore};
use donna_storage::{LocalStore, StoredMemory};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Key, Layout, RichText, ScrollArea, Sense,
    UiBuilder, Vec2,
};
use egui_phosphor::Variant as PhosphorVariant;
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
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
/// Microsoft sync percentages and the running-task count are read from
/// SQLite; throttling avoids hitting the database on every single UI frame
/// (up to 60x/sec) for values that only meaningfully change a few times a
/// minute.
const ACTIVITY_STATUS_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
/// Fallback heartbeat so background-task/minute-boundary checks
/// (`run_due_local_tasks`, `run_due_microsoft_sync`) keep ticking even while
/// hidden and nothing else is causing a repaint. Real wakeup responsiveness
/// no longer depends on this interval: the listener thread (`ipc.rs`) calls
/// `ctx.request_repaint()` directly the instant a `donna --wakeup` message
/// arrives via `RepaintSignal`, so this can be coarse. It previously
/// rescheduled a full repaint every 250ms forever, in every state including
/// Hidden, which kept the whole app rendering at ~4Hz around the clock.
const WAKEUP_POLL_INTERVAL: Duration = Duration::from_secs(20);
const REMEMBERED_FACTS_CONTEXT_PROMPT: &str =
    include_str!("../../../assets/prompts/context/remembered_facts.md");
const CURRENT_OPEN_TODOS_CONTEXT_PROMPT: &str =
    include_str!("../../../assets/prompts/context/current_open_todos.md");
const CURRENT_DATE_TIME_CONTEXT_PROMPT: &str =
    include_str!("../../../assets/prompts/context/current_date_time.md");
const WELCOME_GREETING_PROMPT: &str =
    include_str!("../../../assets/prompts/context/welcome_greeting.md");
const TOOL_RESULT_FOLLOWUP_PROMPT: &str =
    include_str!("../../../assets/prompts/context/tool_result_followup.md");
const WEEKDAY_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

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
    running_task_count: usize,
    last_activity_status_refresh: Instant,
    input: String,
    input_history: Vec<String>,
    input_history_cursor: Option<usize>,
    input_history_draft: String,
    input_notice: Option<String>,
    input_error: Option<String>,
    pending_exit_confirmation: bool,
    models: ModelRegistry,
    selected_model_id: String,
    model_warmup_in_progress: bool,
    model_warmup_model_id: Option<String>,
    model_warmup_receiver: Option<Receiver<Result<(), String>>>,
    warmed_model_ids: HashSet<String>,
    avatar_manager: AvatarManager,
    state: DonnaState,
    response_in_progress: bool,
    streaming_response: Option<StreamingResponse>,
    response_started_at: Option<Instant>,
    /// Messages submitted while a response was already in progress. Each is
    /// pushed into `chat` immediately (as a "queued" bubble the user can
    /// cancel) rather than held back invisibly, so the transcript always
    /// reflects everything the user has actually typed; only *sending* it
    /// to the model waits until Donna finishes the current answer.
    queued_prompts: VecDeque<QueuedPrompt>,
    name_prompt_asked: bool,
    name_prompt_pending: bool,
    approval_pending: bool,
    attention: AttentionUiState,
    idle_frame: u8,
    last_idle_change: Instant,
    hide_requested: Arc<AtomicBool>,
    wakeup_receiver: Option<Arc<Mutex<Receiver<IpcEvent>>>>,
}

/// A user message submitted while Donna was still answering a previous one.
/// Its chat bubble already exists (pushed at submit time); this just tracks
/// which bubble it is and its raw text, so it can be sent to the model, or
/// cancelled and removed, once it reaches the front of the queue.
struct QueuedPrompt {
    message_id: u64,
    text: String,
}

struct StreamingResponse {
    receiver: Receiver<ChatWorkerEvent>,
    message_id: u64,
    text: String,
    placeholder: String,
    user_message: String,
    /// Set once a tool call has been executed and a second model round-trip
    /// was kicked off to let the model phrase the final answer from the
    /// tool's raw output, instead of showing that raw output verbatim.
    tool_followup: Option<ToolFollowup>,
    selection: Option<donna_ai::ProviderSelection>,
    auth_material: Option<String>,
}

/// Carries the raw tool result through the second round-trip so it can be
/// shown immediately (good interim UX while a local model is still slow to
/// respond) and used as a fallback if that round-trip errors, times out, or
/// the model ignores the "don't call a tool" instruction and emits another
/// tool call instead of prose.
struct ToolFollowup {
    raw_tool_result: String,
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
        repaint_signal: RepaintSignal,
    ) -> Self {
        Self::new_with_config_path_and_hide_signal(
            creation,
            AppConfig::default_path(),
            hide_requested,
            Some(wakeup_receiver),
            repaint_signal,
        )
    }

    fn new_with_config_path(creation: &eframe::CreationContext<'_>, config_path: PathBuf) -> Self {
        Self::new_with_config_path_and_hide_signal(
            creation,
            config_path,
            Arc::new(AtomicBool::new(false)),
            None,
            new_repaint_signal(),
        )
    }

    fn new_with_config_path_and_hide_signal(
        creation: &eframe::CreationContext<'_>,
        config_path: PathBuf,
        hide_requested: Arc<AtomicBool>,
        wakeup_receiver: Option<Arc<Mutex<Receiver<IpcEvent>>>>,
        repaint_signal: RepaintSignal,
    ) -> Self {
        set_repaint_context(&repaint_signal, creation.egui_ctx.clone());
        let (mut config, config_notice) = AppConfig::load_or_default_at(&config_path);
        donna_core::time::configure_time_format(config.ui.time_format);
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
            running_task_count: 0,
            last_activity_status_refresh: Instant::now(),
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
            model_warmup_in_progress: false,
            model_warmup_model_id: None,
            model_warmup_receiver: None,
            warmed_model_ids: HashSet::new(),
            avatar_manager: AvatarManager::new(),
            state: DonnaState::Idle,
            response_in_progress: false,
            streaming_response: None,
            response_started_at: None,
            queued_prompts: VecDeque::new(),
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
        app.trigger_model_warmup(&app.selected_model_id.clone());
        // Only greet with a model-generated message on a real app launch
        // (`wakeup_receiver` is only `Some` there) — not for the
        // programmatic/test constructors, which need a clean, predictable
        // chat session to assert against.
        if app.wakeup_receiver.is_some() {
            app.start_welcome_greeting();
        }
        app
    }

    fn cycle_model(&mut self) {
        if let Some(next_model) = self.models.next_after(&self.selected_model_id) {
            let next_model_id = next_model.id.clone();
            self.selected_model_id = next_model_id.clone();
            self.config.ai.chat.selected_model = self.selected_model_id.clone();

            if let Err(error) = self.config.save_to_path(&self.config_path) {
                self.config_notice = Some(error.to_string());
            }
            self.trigger_model_warmup(&next_model_id);
        }
    }

    fn trigger_model_warmup(&mut self, model_id: &str) {
        if self.model_warmup_in_progress || self.warmed_model_ids.contains(model_id) {
            return;
        }
        let Some(model) = self.models.model_by_id(model_id).cloned() else {
            return;
        };
        if model.provider != "ollama" {
            return;
        }

        eprintln!("donna ui: warming up Ollama model ({model_id})");
        let (sender, receiver) = mpsc::channel();
        self.model_warmup_in_progress = true;
        self.model_warmup_model_id = Some(model_id.to_owned());
        self.model_warmup_receiver = Some(receiver);
        thread::spawn(move || {
            let result = OllamaProvider.warm_up(&model).map_err(|error| error.to_string());
            let _ = sender.send(result);
        });
    }

    fn poll_model_warmup(&mut self) {
        let Some(receiver) = self.model_warmup_receiver.as_ref() else {
            return;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => {
                Err("model warm-up worker disconnected".to_owned())
            }
        };

        self.model_warmup_in_progress = false;
        self.model_warmup_receiver = None;
        let Some(model_id) = self.model_warmup_model_id.take() else {
            return;
        };
        match result {
            Ok(()) => {
                eprintln!("donna ui: Ollama model warm-up finished ({model_id})");
                self.warmed_model_ids.insert(model_id);
            }
            Err(error) => {
                eprintln!("donna ui: Ollama model warm-up failed ({model_id}): {error}");
            }
        }
    }

    fn is_selected_model_warming_up(&self) -> bool {
        self.model_warmup_in_progress
            && self.model_warmup_model_id.as_deref() == Some(self.selected_model_id.as_str())
    }

    fn is_selected_model_warmed_up(&self) -> bool {
        self.warmed_model_ids.contains(&self.selected_model_id)
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
                self.remember_submitted_input(&input);
                self.pending_exit_confirmation = false;
                self.state = DonnaState::Idle;
                let Some(message_id) = self.chat.push_user_message(message.as_str()) else {
                    return;
                };
                if self.response_in_progress {
                    self.queued_prompts.push_back(QueuedPrompt {
                        message_id,
                        text: message,
                    });
                    return;
                }
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
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
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
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(WAKEUP_POLL_INTERVAL);
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

    /// Sends the next queued prompt (if any) to the model now that Donna
    /// has finished her current answer, in the order the user submitted
    /// them (FIFO). No-op when the queue is empty.
    fn start_next_queued_prompt(&mut self) {
        let Some(queued) = self.queued_prompts.pop_front() else {
            return;
        };
        self.persist_structured_chat_records(&queued.text);
        if !self.name_prompt_asked && !self.knows_user_name() {
            self.name_prompt_asked = true;
            self.name_prompt_pending = true;
        }
        self.start_chat_response();
    }

    /// Cancels a still-queued prompt: removes its bubble from the transcript
    /// and drops it from the queue. No-op if it already started processing
    /// (it will have left the queue by then) or doesn't exist.
    fn cancel_queued_prompt(&mut self, message_id: u64) {
        let Some(index) = self
            .queued_prompts
            .iter()
            .position(|queued| queued.message_id == message_id)
        else {
            return;
        };
        self.queued_prompts.remove(index);
        self.chat.remove_message(message_id);
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
                self.config_notice = Some(error);
                return;
            }
        };
        // Include the previous user turn along with the latest one: keyword
        // corrections (`wants_attendees`, today/tomorrow date-range fixups)
        // only see this combined string, and a short follow-up like "which
        // are the attendees of that meeting?" carries no date or name of its
        // own — the context that answers it (e.g. "today", a person's name)
        // lives in the turn before. Without it, the follow-up's tool call
        // falls back to the model's own guessed date range, which is
        // unreliable.
        let user_message = {
            let recent_user_messages: Vec<&str> = self
                .chat
                .messages()
                .iter()
                .rev()
                .filter(|message| message.speaker == Speaker::User)
                .take(2)
                .map(|message| message.text.as_str())
                .collect();
            recent_user_messages
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n")
        };

        eprintln!("donna ui: chat input: {user_message}");

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
            tool_followup: None,
            selection: Some(selection.clone()),
            auth_material: auth_material.clone(),
        });
        self.response_in_progress = true;
        self.response_started_at = Some(Instant::now());
        thread::spawn(move || run_chat_worker(selection, request, auth_material, sender));
    }

    /// Replaces the static placeholder welcome message with one the model
    /// generates fresh each launch, so the opening line isn't always the
    /// same handful of hardcoded strings. Fails silently (keeping the
    /// static placeholder already in the chat) if no model/auth is
    /// available, since a missing greeting is not worth surfacing as an
    /// error on startup.
    fn start_welcome_greeting(&mut self) {
        let Some(message_id) = self.chat.messages().first().map(|message| message.id) else {
            return;
        };
        let Ok(selection) = ProviderCatalog::select_chat_model(&self.models, &self.selected_model_id)
        else {
            return;
        };
        let Ok(auth_material) = load_model_auth_material(&selection) else {
            return;
        };

        let prompt = load_system_prompt(&self.config);
        if let Some(notice) = prompt.notice {
            self.config_notice = Some(notice);
        }
        let mut system_prompt = self.system_prompt_with_memories(prompt.content);
        system_prompt.push_str(WELCOME_GREETING_PROMPT);
        let request = AiRequest::new(system_prompt)
            .with_message(AiMessage::trusted(AiRole::User, "(app startup)"));

        let (sender, receiver) = mpsc::channel();
        self.streaming_response = Some(StreamingResponse {
            receiver,
            message_id,
            text: String::new(),
            placeholder: String::new(),
            user_message: String::new(),
            tool_followup: None,
            selection: None,
            auth_material: None,
        });
        self.response_in_progress = true;
        self.response_started_at = Some(Instant::now());
        thread::spawn(move || run_chat_worker(selection, request, auth_material, sender));
    }

    fn system_prompt_with_memories(&mut self, mut system_prompt: String) -> String {
        self.append_current_date_time_to_prompt(&mut system_prompt);
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

    fn append_current_date_time_to_prompt(&self, system_prompt: &mut String) {
        let Some(now) = unix_now_seconds() else {
            return;
        };
        system_prompt.push_str(CURRENT_DATE_TIME_CONTEXT_PROMPT);
        system_prompt.push_str(&current_date_time_line(now));
        system_prompt.push('\n');
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
                    if let Some(followup) = streaming.tool_followup.take() {
                        let final_text = finalize_tool_followup_text(text, &followup.raw_tool_result);
                        eprintln!("donna ui: chat output: {final_text}");
                        self.chat
                            .replace_message_text(streaming.message_id, final_text);
                        finished = true;
                        continue;
                    }

                    if let Some(tool_result) = execute_tool_call_from_model(
                        self.store.as_ref(),
                        &text,
                        &streaming.user_message,
                    ) {
                        eprintln!("donna harness: chat model raw tool call: {text}");
                        self.chat
                            .replace_message_text(streaming.message_id, tool_result.clone());

                        let followup_selection = streaming
                            .selection
                            .clone()
                            .filter(|selection| selection.family != ProviderFamily::Mock);
                        match followup_selection {
                            Some(selection) => {
                                let request = build_tool_followup_request(
                                    &streaming.user_message,
                                    &tool_result,
                                );
                                let auth_material = streaming.auth_material.clone();
                                let (sender, receiver) = mpsc::channel();
                                streaming.receiver = receiver;
                                streaming.text.clear();
                                streaming.placeholder.clear();
                                streaming.tool_followup = Some(ToolFollowup {
                                    raw_tool_result: tool_result,
                                });
                                thread::spawn(move || {
                                    run_chat_worker(selection, request, auth_material, sender)
                                });
                            }
                            None => {
                                eprintln!("donna ui: chat output: {tool_result}");
                                finished = true;
                            }
                        }
                        continue;
                    }
                    if streaming.text.is_empty() {
                        let text = if text.trim().is_empty() {
                            "The selected model returned an empty response.".to_owned()
                        } else {
                            humanize_model_todo_leak(self.store.as_ref(), text)
                        };
                        eprintln!("donna ui: chat output: {text}");
                        self.chat.replace_message_text(streaming.message_id, text);
                    } else {
                        eprintln!("donna ui: chat output: {}", streaming.text);
                    }
                    finished = true;
                }
                ChatWorkerEvent::Error(error) => {
                    if let Some(followup) = streaming.tool_followup.take() {
                        eprintln!(
                            "donna ui: tool follow-up call failed ({error}); keeping raw tool result"
                        );
                        self.chat
                            .replace_message_text(streaming.message_id, followup.raw_tool_result);
                    } else {
                        self.chat.remove_message(streaming.message_id);
                        self.config_notice = Some(error);
                    }
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
            self.start_next_queued_prompt();
        } else {
            // While a tool follow-up call is in flight, the bubble already
            // shows the raw tool result as a meaningful interim answer —
            // don't blank it out with "..." dots until the model actually
            // starts streaming its rephrased reply.
            if streaming.text.is_empty() && streaming.tool_followup.is_none() {
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
        self.refresh_activity_status_throttled();
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
            self.refresh_activity_status();
            match result {
                Ok(()) => eprintln!("donna ui: Microsoft sync worker finished"),
                Err(error) => {
                    eprintln!("donna ui: Microsoft sync worker failed: {error}");
                    self.config_notice = Some(error);
                }
            }
        }
    }

    /// Reads Microsoft sync progress and the running-task count from SQLite
    /// at most once per [`ACTIVITY_STATUS_REFRESH_INTERVAL`], since this is
    /// otherwise called every UI frame and these values rarely change that
    /// fast — keeping database work off the per-frame hot path avoids the
    /// UI stalling on synchronous SQLite reads while a background job is
    /// also writing to the same database.
    fn refresh_activity_status_throttled(&mut self) {
        if self.last_activity_status_refresh.elapsed() < ACTIVITY_STATUS_REFRESH_INTERVAL {
            return;
        }
        self.last_activity_status_refresh = Instant::now();
        self.refresh_activity_status();
    }

    fn refresh_activity_status(&mut self) {
        let Some(store) = self.store.as_ref() else {
            self.microsoft_sync_status = MicrosoftSyncUiStatus::default();
            self.running_task_count = 0;
            return;
        };

        self.microsoft_sync_status = MicrosoftSyncUiStatus {
            outlook_percent: Self::runtime_percent(store, "microsoft.sync.progress.outlook"),
            teams_percent: Self::runtime_percent(store, "microsoft.sync.progress.teams"),
            calendar_percent: Self::runtime_percent(store, "microsoft.sync.progress.calendar"),
        };
        self.running_task_count = store.running_task_run_count().unwrap_or(0);
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

/// Builds the compact second round-trip request that hands a tool's raw
/// output back to the model so it can phrase the final answer. Deliberately
/// skips the full system prompt, tool catalog, and chat history that the
/// first round-trip sends: local models are slow and have a small context
/// window, so keeping this request minimal keeps the extra round-trip cheap.
fn build_tool_followup_request(user_message: &str, tool_result: &str) -> AiRequest {
    let content = format!("The user asked: \"{user_message}\"\n\nTool result:\n{tool_result}");
    AiRequest::new(TOOL_RESULT_FOLLOWUP_PROMPT.to_owned())
        .with_message(AiMessage::trusted(AiRole::User, content))
}

/// True when text still looks like a tool call (JSON-ish) rather than the
/// prose reply the follow-up prompt asked for. Local models sometimes ignore
/// the "don't call a tool" instruction on the second round-trip; this is a
/// cheap syntactic check (not a real parse, to avoid re-executing a tool
/// call and risking a duplicate side effect for mutating tools).
/// Removes any "[event_id: N]" reference tags before showing a message to
/// the user — see `tag_calendar_followup_with_event_id`. The tag stays in
/// the stored message text (that's what lets a later turn resolve "that
/// meeting" deterministically), but it's an internal handle, never something
/// the user should see.
fn strip_event_reference_tags(text: &str) -> String {
    const PREFIX: &str = "[event_id: ";
    if !text.contains(PREFIX) {
        return text.to_owned();
    }
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(PREFIX) {
        result.push_str(rest[..start].trim_end_matches(' '));
        let after = &rest[start + PREFIX.len()..];
        rest = match after.find(']') {
            Some(end) => &after[end + 1..],
            None => break,
        };
    }
    result.push_str(rest);
    result.trim_end().to_owned()
}

fn looks_like_tool_call_json(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with('{') || trimmed.contains("```json") || trimmed.contains("\"arguments\"")
}

/// Picks the text to show after the tool follow-up round-trip: the model's
/// rephrased answer if it looks like a genuine reply, otherwise the raw tool
/// result as a safe fallback (empty response, or the model repeating a tool
/// call instead of prose). When the answer is about a calendar appointment,
/// also tags it with that appointment's event_id (see
/// `tag_calendar_followup_with_event_id`) so a later follow-up can resolve
/// "that meeting" deterministically instead of re-guessing search filters.
fn finalize_tool_followup_text(model_text: String, raw_tool_result: &str) -> String {
    let trimmed = model_text.trim();
    if trimmed.is_empty() || looks_like_tool_call_json(trimmed) {
        return raw_tool_result.to_owned();
    }
    tag_calendar_followup_with_event_id(trimmed.to_owned(), raw_tool_result)
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
        self.poll_model_warmup();
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

fn current_date_time_line(now: i64) -> String {
    let days = now.div_euclid(86_400);
    let seconds_of_day = now.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day / 60) % 60;
    let second = seconds_of_day % 60;
    let weekday = WEEKDAY_NAMES[((days + 4).rem_euclid(7)) as usize];
    format!(
        "Current date and time (UTC): {year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z ({weekday}). Current unix timestamp (seconds since epoch): {now}."
    )
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
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        if response.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
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
        let mut prompt_to_cancel = None;
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .max_height((inner_size.y - input_height - attention_height).max(24.0))
            .show(&mut chat_ui, |ui| {
                ui.set_width(inner_size.x);
                for message in self.chat.messages() {
                    let queued = self
                        .queued_prompts
                        .iter()
                        .any(|queued| queued.message_id == message.id);
                    let display_text = strip_event_reference_tags(&message.text);
                    let cancelled = render_message(
                        ui,
                        message.speaker,
                        &display_text,
                        inner_size.x,
                        &self.config,
                        queued,
                    );
                    if cancelled {
                        prompt_to_cancel = Some(message.id);
                    }
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
        if let Some(message_id) = prompt_to_cancel {
            self.cancel_queued_prompt(message_id);
        }

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
