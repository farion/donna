use crate::avatar::{AvatarManager, AvatarState};
use crate::chat::{ChatSession, Speaker};
use crate::command::{AppCommand, ParsedInput, parse_input};
use crate::config::AppConfig;
use crate::model::ModelRegistry;
use crate::storage::LocalStore;
use eframe::egui::{
    self, Align, Button, CentralPanel, Color32, CornerRadius, FontId, Frame, Key, Layout, Margin,
    RichText, ScrollArea, TextEdit, Vec2,
};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const AVATAR_MIN_SIDE: f32 = 220.0;
const AVATAR_MAX_SIDE: f32 = 480.0;
const CHAT_WIDTH_RATIO: f32 = 0.8;
const CHAT_MIN_WIDTH: f32 = 280.0;
const ROW_GAP: f32 = 24.0;
const COMPACT_ROW_GAP: f32 = 16.0;
const AVATAR_FRAME_MARGIN: f32 = 32.0;
const CHAT_FRAME_MARGIN: f32 = 28.0;
const SHELL_FRAME_MARGIN: f32 = AVATAR_FRAME_MARGIN + CHAT_FRAME_MARGIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DonnaState {
    Idle,
    Command,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ShellLayout {
    avatar_side: f32,
    chat_width: f32,
    chat_height: f32,
    gap: f32,
    stacked: bool,
}

pub struct DonnaApp {
    config_path: PathBuf,
    config: AppConfig,
    config_notice: Option<String>,
    chat: ChatSession,
    _store: Option<LocalStore>,
    input: String,
    models: ModelRegistry,
    selected_model_id: String,
    avatar_manager: AvatarManager,
    state: DonnaState,
    idle_frame: u8,
    last_idle_change: Instant,
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
        apply_style(&creation.egui_ctx);

        Self {
            config_path,
            config,
            config_notice,
            chat: ChatSession::with_welcome(),
            _store: store,
            input: String::new(),
            models,
            selected_model_id,
            avatar_manager: AvatarManager::new(),
            state: DonnaState::Idle,
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

        match parse_input(&input) {
            ParsedInput::Empty => {}
            ParsedInput::Message(message) => {
                self.state = DonnaState::Idle;
                self.chat.push_user_message(message);
                self.chat.push_donna_message(
                    "Provider integration is not connected yet. I kept this exchange in memory only.",
                );
            }
            ParsedInput::Command(AppCommand::Exit) => {
                self.state = DonnaState::Command;
                self.chat.push_user_message(input);
                self.chat.push_donna_message("Exit requested.");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ParsedInput::Command(AppCommand::Hide) => {
                self.state = DonnaState::Hidden;
                self.chat.push_user_message(input);
                self.chat.push_donna_message(
                    "Minimizing Donna. If your desktop ignores the request, hide the window from the window manager; Donna keeps running.",
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            ParsedInput::Command(AppCommand::Unknown(command)) => {
                self.state = DonnaState::Command;
                self.chat.push_user_message(input);
                self.chat
                    .push_donna_message(format!("Unknown command: /{command}"));
            }
        }
    }

    fn refresh_idle_frame(&mut self, ctx: &egui::Context) {
        if self.state != DonnaState::Idle {
            return;
        }

        if self.last_idle_change.elapsed() >= Duration::from_millis(800) {
            self.idle_frame = random_idle_frame();
            self.last_idle_change = Instant::now();
        }

        ctx.request_repaint_after(Duration::from_millis(250));
    }

    fn avatar_state(&self) -> AvatarState {
        match self.state {
            DonnaState::Idle => AvatarState::Idle(self.idle_frame),
            DonnaState::Command => AvatarState::Command,
            DonnaState::Hidden => AvatarState::Attention,
        }
    }

    fn state_label(&self) -> &'static str {
        match self.state {
            DonnaState::Idle => "Idle",
            DonnaState::Command => "Command",
            DonnaState::Hidden => "Hidden",
        }
    }
}

impl eframe::App for DonnaApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Tab)) {
            self.cycle_model();
        }

        self.refresh_idle_frame(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        CentralPanel::default()
            .frame(Frame::NONE.fill(Color32::from_rgb(242, 244, 239)))
            .show_inside(ui, |ui| {
                ui.add_space(14.0);
                ui.horizontal_centered(|ui| {
                    ui.heading(RichText::new("Donna").font(FontId::proportional(24.0)));
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new("local-first assistant shell")
                            .font(FontId::proportional(14.0))
                            .color(Color32::from_rgb(89, 96, 103)),
                    );
                });
                ui.add_space(16.0);

                let layout = shell_layout(ui.available_size());
                if layout.stacked {
                    ui.vertical_centered(|ui| {
                        self.render_avatar(ui, Vec2::splat(layout.avatar_side));
                        ui.add_space(layout.gap);
                        self.render_chat(
                            ui,
                            Vec2::new(layout.chat_width, layout.chat_height),
                            &ctx,
                        );
                    });
                } else {
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        self.render_avatar(ui, Vec2::splat(layout.avatar_side));
                        ui.add_space(layout.gap);
                        self.render_chat(
                            ui,
                            Vec2::new(layout.chat_width, layout.chat_height),
                            &ctx,
                        );
                    });
                }
            });
    }
}

impl DonnaApp {
    fn render_avatar(&mut self, ui: &mut egui::Ui, size: Vec2) {
        Frame::NONE
            .fill(Color32::from_rgb(232, 236, 230))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(16))
            .show(ui, |ui| {
                ui.set_min_size(size);
                ui.set_max_size(size);

                let character = self.config.avatar.character.as_str();
                if let Some(texture) =
                    self.avatar_manager
                        .texture_for(ui.ctx(), character, self.avatar_state())
                {
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Image::new((texture.id(), size * 0.82)));
                    });
                } else {
                    ui.centered_and_justified(|ui| {
                        ui.label(RichText::new("Donna").font(FontId::proportional(28.0)));
                    });
                }
            });
    }

    fn render_chat(&mut self, ui: &mut egui::Ui, size: Vec2, ctx: &egui::Context) {
        Frame::NONE
            .fill(Color32::from_rgb(252, 252, 249))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.set_min_size(size);
                ui.set_max_size(size);

                let input_height = 88.0;
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .max_height((size.y - input_height).max(120.0))
                    .show(ui, |ui| {
                        for message in self.chat.messages() {
                            self.render_message(ui, message.speaker, &message.text, size.x);
                            ui.add_space(8.0);
                        }

                        if let Some(notice) = &self.config_notice {
                            ui.label(
                                RichText::new(notice)
                                    .font(FontId::proportional(12.0))
                                    .color(Color32::from_rgb(148, 75, 45)),
                            );
                        }
                    });

                ui.separator();
                self.render_chat_bar(ui, ctx);
            });
    }

    fn render_message(&self, ui: &mut egui::Ui, speaker: Speaker, text: &str, chat_width: f32) {
        let color = match speaker {
            Speaker::Donna => parse_hex_color(&self.config.ui.donna_message_color)
                .unwrap_or(Color32::from_rgb(238, 245, 255)),
            Speaker::User => parse_hex_color(&self.config.ui.user_message_color)
                .unwrap_or(Color32::from_rgb(234, 247, 239)),
        };
        let layout = match speaker {
            Speaker::Donna => Layout::left_to_right(Align::Min),
            Speaker::User => Layout::right_to_left(Align::Min),
        };

        ui.with_layout(layout, |ui| {
            Frame::NONE
                .fill(color)
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(12, 9))
                .show(ui, |ui| {
                    ui.set_max_width(chat_width * 0.72);
                    ui.label(RichText::new(text).font(FontId::proportional(14.0)));
                });
        });
    }

    fn render_chat_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.state_label())
                    .font(FontId::proportional(13.0))
                    .color(Color32::from_rgb(72, 83, 92)),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(self.models.selected_label(&self.selected_model_id))
                        .font(FontId::proportional(13.0))
                        .color(Color32::from_rgb(72, 83, 92)),
                );
            });
        });

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let send_width = 64.0;
            let control_gap = ui.spacing().item_spacing.x;
            let available_width = (ui.available_width() - send_width - control_gap).max(96.0);
            let response = ui.add_sized(
                [available_width, 34.0],
                TextEdit::singleline(&mut self.input)
                    .hint_text("Message Donna")
                    .desired_width(available_width),
            );

            let enter_pressed = response.lost_focus()
                && ui.input(|input| input.key_pressed(Key::Enter) && !input.modifiers.shift);
            let send_clicked = ui.add_sized([64.0, 34.0], Button::new("Send")).clicked();

            if enter_pressed || send_clicked {
                self.submit_input(ctx);
            }
        });
    }
}

fn apply_style(ctx: &egui::Context) {
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
        style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
        style.visuals.widgets.active.corner_radius = CornerRadius::same(6);
    });
}

fn shell_layout(available: Vec2) -> ShellLayout {
    let height_limited_avatar = (available.y - AVATAR_FRAME_MARGIN)
        .clamp(AVATAR_MIN_SIDE, AVATAR_MAX_SIDE)
        .min((available.x - AVATAR_FRAME_MARGIN).max(AVATAR_MIN_SIDE));
    let roomy_chat_width = height_limited_avatar * CHAT_WIDTH_RATIO;
    let roomy_total = height_limited_avatar + roomy_chat_width + ROW_GAP + SHELL_FRAME_MARGIN;

    if roomy_total <= available.x {
        return ShellLayout {
            avatar_side: height_limited_avatar,
            chat_width: roomy_chat_width,
            chat_height: height_limited_avatar,
            gap: ROW_GAP,
            stacked: false,
        };
    }

    let row_content_width = available.x - COMPACT_ROW_GAP - SHELL_FRAME_MARGIN;
    let compact_avatar = (row_content_width / (1.0 + CHAT_WIDTH_RATIO))
        .min(height_limited_avatar)
        .max(0.0);
    let compact_chat_width = compact_avatar * CHAT_WIDTH_RATIO;

    if compact_avatar >= AVATAR_MIN_SIDE && compact_chat_width >= CHAT_MIN_WIDTH {
        return ShellLayout {
            avatar_side: compact_avatar,
            chat_width: compact_chat_width,
            chat_height: compact_avatar,
            gap: COMPACT_ROW_GAP,
            stacked: false,
        };
    }

    let stacked_width = (available.x - CHAT_FRAME_MARGIN).max(0.0);
    let stacked_avatar = height_limited_avatar.min(available.x - AVATAR_FRAME_MARGIN);

    ShellLayout {
        avatar_side: stacked_avatar.max(AVATAR_MIN_SIDE),
        chat_width: stacked_width,
        chat_height: height_limited_avatar,
        gap: COMPACT_ROW_GAP,
        stacked: true,
    }
}

fn parse_hex_color(value: &str) -> Option<Color32> {
    let hex = value.strip_prefix('#')?;

    if hex.len() != 6 {
        return None;
    }

    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color32::from_rgb(red, green, blue))
}

fn random_idle_frame() -> u8 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos())
        .unwrap_or(0);
    ((nanos % 3) + 1) as u8
}

#[cfg(test)]
mod tests;
