use crate::avatar::AvatarManager;
use crate::chat::ChatSession;
use crate::command::{AppCommand, ParsedInput, parse_input};
use crate::config::AppConfig;
use crate::memory::{MemoryExtractor, SensitiveMemoryApproval};
use crate::model::ModelRegistry;
use crate::storage::LocalStore;
use crate::tasks::TaskRunnerState;
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Frame, Key, Margin, RichText, ScrollArea, Vec2,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

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
use layout::{CHAT_INNER_MARGIN, avatar_image_size, shell_layout};
use state::{AvatarSignals, DonnaState, random_idle_frame, resolve_state};
use ui_style::{apply_style, palette_for, render_message};

pub struct DonnaApp {
    config_path: PathBuf,
    config: AppConfig,
    config_notice: Option<String>,
    chat: ChatSession,
    store: Option<LocalStore>,
    memory_extractor: MemoryExtractor,
    sensitive_memory_reviews: memory_review::SensitiveMemoryReviews,
    task_runner_state: TaskRunnerState,
    input: String,
    input_notice: Option<String>,
    input_error: Option<String>,
    pending_exit_confirmation: bool,
    models: ModelRegistry,
    selected_model_id: String,
    avatar_manager: AvatarManager,
    state: DonnaState,
    response_in_progress: bool,
    approval_pending: bool,
    attention: AttentionUiState,
    idle_frame: u8,
    last_idle_change: Instant,
}

pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Donna")
            .with_inner_size([1220.0, 960.0])
            .with_min_inner_size([720.0, 480.0])
            .with_transparent(true)
            .with_decorations(false)
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

    fn new_with_config_path(creation: &eframe::CreationContext<'_>, config_path: PathBuf) -> Self {
        let (mut config, config_notice) = AppConfig::load_or_default_at(&config_path);
        let (store, storage_notice) = match LocalStore::open(&config.data.database_path) {
            Ok(store) => (Some(store), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let config_notice = config_notice.or(storage_notice);
        let models = ModelRegistry::from_config(&config);
        let selected_model_id = models
            .normalized_selected_id(&config.ai.chat.selected_model)
            .unwrap_or_else(|| config.ai.chat.selected_model.clone());

        config.ai.chat.selected_model = selected_model_id.clone();
        apply_style(&creation.egui_ctx, config.ui.theme);

        Self {
            config_path,
            memory_extractor: MemoryExtractor::from_config(&config.memory),
            config,
            config_notice,
            chat: ChatSession::with_welcome(),
            store,
            task_runner_state: TaskRunnerState::running(),
            sensitive_memory_reviews: memory_review::SensitiveMemoryReviews::default(),
            input: String::new(),
            input_notice: None,
            input_error: None,
            pending_exit_confirmation: false,
            models,
            selected_model_id,
            avatar_manager: AvatarManager::new(),
            state: DonnaState::Idle,
            response_in_progress: false,
            approval_pending: false,
            attention: AttentionUiState::default(),
            idle_frame: random_idle_frame(),
            last_idle_change: Instant::now(),
        }
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
        self.input_notice = None;
        self.input_error = None;

        match parse_input(&input) {
            ParsedInput::Empty => {}
            ParsedInput::Message(message) => {
                self.pending_exit_confirmation = false;
                self.state = DonnaState::Idle;
                self.chat.push_user_message(message.as_str());
                self.response_in_progress = true;
                let memory_note = self.persist_structured_chat_records(&message);
                let model_label = self.models.selected_label(&self.selected_model_id);
                self.chat.push_donna_message(format!(
                    "{model_label} is selected for chat. {memory_note}"
                ));
                self.response_in_progress = false;
            }
            ParsedInput::Command(AppCommand::Exit { confirmed }) => {
                self.handle_exit_command(confirmed, ctx);
            }
            ParsedInput::Command(AppCommand::Hide) => self.handle_hide_command(ctx),
            ParsedInput::Command(AppCommand::ChangeCharacter(character)) => {
                self.handle_change_character_command(character.as_deref());
            }
            ParsedInput::Command(AppCommand::Theme(theme)) => {
                self.handle_theme_command(theme.as_deref(), ctx);
            }
            ParsedInput::Command(AppCommand::Unknown(command)) => {
                self.show_command_error(format!("Unknown command: /{command}"))
            }
        }
    }

    fn handle_exit_command(&mut self, confirmed: bool, ctx: &egui::Context) {
        self.state = DonnaState::Command;
        if !confirmed {
            self.pending_exit_confirmation = true;
            self.input_notice =
                Some("Type /exit confirm to stop Donna and background tasks.".to_owned());
            return;
        }

        self.pending_exit_confirmation = false;
        self.task_runner_state.stop();
        self.input_notice = Some("Exit confirmed. Stopping Donna.".to_owned());
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn handle_hide_command(&mut self, ctx: &egui::Context) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Hidden;
        self.input_notice = Some(
            "Donna is minimized and background tasks keep running; your compositor may ignore minimize requests."
                .to_owned(),
        );
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

    fn show_command_error(&mut self, error: impl Into<String>) {
        self.pending_exit_confirmation = false;
        self.state = DonnaState::Command;
        self.input_error = Some(error.into());
    }

    fn refresh_idle_frame(&mut self, ctx: &egui::Context) {
        if self.visual_state() != DonnaState::Idle {
            return;
        }

        if self.last_idle_change.elapsed() >= Duration::from_millis(800) {
            self.idle_frame = random_idle_frame();
            self.last_idle_change = Instant::now();
        }

        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn avatar_state(&self) -> crate::avatar::AvatarState {
        self.visual_state().avatar_state(self.idle_frame)
    }

    fn state_label(&self) -> &'static str {
        self.visual_state().label()
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
}

impl eframe::App for DonnaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        let ctx = ui.ctx().clone();
        let available = ui.available_size();
        let layout = shell_layout(available);
        let content_width = layout.avatar_width + layout.gap + layout.chat_width;
        let content_height = layout.avatar_height.max(layout.chat_height);
        let left_space = ((available.x - content_width) / 2.0).max(0.0);
        let top_space = ((available.y - content_height) / 2.0).max(0.0);

        ui.add_space(top_space);
        ui.horizontal(|ui| {
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

impl DonnaApp {
    fn render_avatar(&mut self, ui: &mut egui::Ui, size: Vec2) {
        let palette = palette_for(ui.ctx().theme());
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
        } else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Donna",
                FontId::proportional(28.0),
                palette.heading_text,
            );
        }
    }

    fn render_chat(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        let palette = palette_for(ui.ctx().theme());
        let margin = CHAT_INNER_MARGIN;
        let inner_size = Vec2::new(
            (size.x - margin * 2.0).max(0.0),
            (size.y - margin * 2.0).max(0.0),
        );
        Frame::NONE
            .fill(palette.chat_fill)
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(CHAT_INNER_MARGIN as i8))
            .show(ui, |ui| {
                ui.set_min_size(inner_size);
                ui.set_max_size(inner_size);

                let input_height = chat_bar_reserved_height(inner_size.x);
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .max_height((inner_size.y - input_height).max(24.0))
                    .show(ui, |ui| {
                        self.attention
                            .render(ui, self.store.as_ref(), &mut self.config_notice);
                        if self.attention.has_active_item() {
                            ui.add_space(8.0);
                        }

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

                ui.separator();
                self.render_chat_bar(ui, ctx);
            });
    }
}

#[cfg(test)]
mod tests;
