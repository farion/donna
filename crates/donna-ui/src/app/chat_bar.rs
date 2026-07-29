use super::ui_style::palette_for;
use super::{DonnaApp, status};
use eframe::egui::ecolor::Hsva;
use eframe::egui::{self, Align, Color32, FontId, Key, Label, Layout, Margin, RichText, TextEdit};
use egui_phosphor::regular::{AIRPLANE_TAKEOFF, CHECK, CIRCLE_NOTCH, MICROSOFT_TEAMS_LOGO};

/// Seconds for one full hue rotation of an active-state icon's rainbow
/// animation.
const ICON_RAINBOW_CYCLE_SECONDS: f64 = 2.5;

const COMPACT_CHAT_BAR_WIDTH: f32 = 180.0;
const COMPACT_CHAT_BAR_HEIGHT: f32 = 88.0;
const ROOMY_CHAT_BAR_HEIGHT: f32 = 88.0;
const CHAT_CONTROL_HEIGHT: f32 = 34.0;
const INPUT_TEXT_MARGIN: Margin = Margin {
    left: 12,
    right: 12,
    top: 8,
    bottom: 8,
};

pub(super) fn chat_bar_reserved_height(
    inner_width: f32,
    _input: &str,
    _ctx: Option<&egui::Context>,
) -> f32 {
    if is_compact_chat_bar(inner_width) {
        COMPACT_CHAT_BAR_HEIGHT
    } else {
        ROOMY_CHAT_BAR_HEIGHT
    }
}

fn is_compact_chat_bar(inner_width: f32) -> bool {
    inner_width < COMPACT_CHAT_BAR_WIDTH
}

impl DonnaApp {
    pub(super) fn render_chat_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let compact = is_compact_chat_bar(ui.available_width());

        self.render_chat_status(ui, compact);
        ui.add_space(6.0);

        if compact {
            self.render_compact_chat_input(ui, ctx);
        } else {
            self.render_roomy_chat_input(ui, ctx);
        }

        self.render_input_feedback(ui);
    }

    fn render_chat_status(&self, ui: &mut egui::Ui, compact: bool) {
        let palette = palette_for(ui.ctx().theme());
        let state_label = self.state_label();
        let status_label = status::status_label(
            &state_label,
            self.store.as_ref(),
            self.config.offline.show_stale_data_warnings,
        );
        let model_label = self.models.selected_label(&self.selected_model_id);

        if compact {
            let width = ui.available_width();
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add(Label::new(chat_bar_text(status_label, palette.notice_text)).wrap());
                self.render_microsoft_sync_icon(ui, 13.0);
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add(Label::new(chat_bar_text(model_label, palette.notice_text)).wrap());
                self.render_model_status_icon(ui, palette.notice_text, 13.0);
            });
            ui.set_min_width(width);
            return;
        }

        ui.horizontal(|ui| {
            ui.label(chat_bar_text(status_label, palette.notice_text));
            self.render_microsoft_sync_icon(ui, 14.0);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.render_model_status_icon(ui, palette.notice_text, 13.0) {
                    ui.add_space(4.0);
                }
                ui.label(chat_bar_text(model_label, palette.notice_text));
            });
        });
    }

    fn render_microsoft_sync_icon(&self, ui: &mut egui::Ui, size: f32) {
        if !self.microsoft_sync_in_progress {
            return;
        }
        ui.add_space(4.0);
        let color = rainbow_color(ui.ctx());
        ui.label(activity_icon(MICROSOFT_TEAMS_LOGO, color, size));
    }

    /// Renders the model warm-up status icon, if any, and reports whether
    /// one was drawn so callers can decide whether to reserve space for it.
    fn render_model_status_icon(&self, ui: &mut egui::Ui, notice_color: Color32, size: f32) -> bool {
        if self.is_selected_model_warming_up() {
            let color = rainbow_color(ui.ctx());
            ui.label(activity_icon(AIRPLANE_TAKEOFF, color, size));
            return true;
        }
        if self.is_selected_model_warmed_up() {
            ui.label(activity_icon(CHECK, notice_color, size));
            return true;
        }
        false
    }

    pub(super) fn render_activity_strip(&self, ui: &mut egui::Ui, compact: bool) {
        let palette = palette_for(ui.ctx().theme());
        let icon_size = if compact { 13.0 } else { 14.0 };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if self.running_task_count > 0 {
                ui.label(activity_icon(CIRCLE_NOTCH, palette.notice_text, icon_size));
            }
        });
    }

    fn render_compact_chat_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.render_chat_input(ui, ctx, "Message");
    }

    fn render_roomy_chat_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        self.render_chat_input(ui, ctx, "Message Donna");
    }

    fn render_chat_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, hint: &str) {
        let width = ui.available_width().max(1.0);
        let response = ui.add_sized(
            [width, CHAT_CONTROL_HEIGHT],
            TextEdit::singleline(&mut self.input)
                .id_salt("chat-input")
                .hint_text(hint)
                .margin(INPUT_TEXT_MARGIN)
                .vertical_align(Align::Center)
                .desired_width(width),
        );

        if !response.has_focus() {
            response.request_focus();
        }
        keep_tab_in_chat_input(ui, &response);
        self.handle_input_history_keys(ctx, &response);
        self.submit_on_enter(ctx, response);
    }

    fn handle_input_history_keys(&mut self, ctx: &egui::Context, response: &egui::Response) {
        if !response.has_focus() {
            return;
        }
        if response.changed() {
            self.input_history_cursor = None;
            self.input_history_draft.clear();
        }

        let up_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::ArrowUp));
        if up_pressed {
            self.show_previous_input_history_item();
            return;
        }

        let down_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::ArrowDown));
        if down_pressed {
            self.show_next_input_history_item();
        }
    }

    fn show_previous_input_history_item(&mut self) {
        if self.input_history.is_empty() {
            return;
        }

        let cursor = match self.input_history_cursor {
            Some(0) => 0,
            Some(cursor) => cursor - 1,
            None => {
                self.input_history_draft = self.input.clone();
                self.input_history.len() - 1
            }
        };
        self.input_history_cursor = Some(cursor);
        self.input = self.input_history[cursor].clone();
    }

    fn show_next_input_history_item(&mut self) {
        let Some(cursor) = self.input_history_cursor else {
            return;
        };

        if cursor + 1 < self.input_history.len() {
            let cursor = cursor + 1;
            self.input_history_cursor = Some(cursor);
            self.input = self.input_history[cursor].clone();
            return;
        }

        self.input_history_cursor = None;
        self.input = std::mem::take(&mut self.input_history_draft);
    }

    fn submit_on_enter(&mut self, ctx: &egui::Context, _response: egui::Response) {
        let enter_pressed =
            ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Enter));

        if enter_pressed {
            self.submit_input(ctx);
        }
    }
}

fn keep_tab_in_chat_input(ui: &mut egui::Ui, response: &egui::Response) {
    ui.memory_mut(|memory| {
        memory.set_focus_lock_filter(
            response.id,
            egui::EventFilter {
                tab: true,
                ..Default::default()
            },
        );
    });
}

fn chat_bar_text(text: impl Into<String>, color: egui::Color32) -> RichText {
    RichText::new(text)
        .font(FontId::proportional(13.0))
        .color(color)
}

/// Computes a smoothly cycling rainbow color for an active-state icon (e.g.
/// syncing, warming up) and keeps the animation going by requesting the next
/// repaint — only called while that icon is actually visible, so idle frames
/// stay untouched.
fn rainbow_color(ctx: &egui::Context) -> Color32 {
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    let time = ctx.input(|input| input.time);
    let hue = (time / ICON_RAINBOW_CYCLE_SECONDS).fract() as f32;
    Hsva::new(hue, 0.75, 0.95, 1.0).into()
}

fn activity_icon(icon: &str, color: egui::Color32, size: f32) -> RichText {
    RichText::new(icon)
        .font(FontId::proportional(size))
        .color(color)
}
