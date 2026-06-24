use super::ui_style::palette_for;
use super::{DonnaApp, status};
use eframe::egui::{self, Align, Button, FontId, Key, Label, Layout, RichText, TextEdit};

const COMPACT_CHAT_BAR_WIDTH: f32 = 180.0;
const COMPACT_CHAT_BAR_HEIGHT: f32 = 128.0;
const ROOMY_CHAT_BAR_HEIGHT: f32 = 88.0;
const CHAT_CONTROL_HEIGHT: f32 = 34.0;
const MIN_ROOMY_INPUT_WIDTH: f32 = 96.0;
const SEND_WIDTH: f32 = 64.0;

pub(super) fn chat_bar_reserved_height(inner_width: f32) -> f32 {
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
        self.render_command_suggestions(ui);

        if compact {
            self.render_compact_chat_input(ui, ctx);
        } else {
            self.render_roomy_chat_input(ui, ctx);
        }

        self.render_input_feedback(ui);
    }

    fn render_chat_status(&self, ui: &mut egui::Ui, compact: bool) {
        let palette = palette_for(ui.ctx().theme());
        let status_label = status::status_label(
            self.state_label(),
            self.store.as_ref(),
            self.config.offline.show_stale_data_warnings,
        );
        let model_label = self.models.selected_label(&self.selected_model_id);

        if compact {
            let width = ui.available_width();
            ui.add(Label::new(chat_bar_text(status_label, palette.notice_text)).wrap());
            ui.add(Label::new(chat_bar_text(model_label, palette.notice_text)).wrap());
            ui.set_min_width(width);
            return;
        }

        ui.horizontal(|ui| {
            ui.label(chat_bar_text(status_label, palette.notice_text));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(chat_bar_text(model_label, palette.notice_text));
            });
        });
    }

    fn render_compact_chat_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let width = ui.available_width().max(1.0);
        let response = ui.add_sized(
            [width, CHAT_CONTROL_HEIGHT],
            TextEdit::singleline(&mut self.input)
                .hint_text("Message")
                .desired_width(width),
        );

        ui.add_space(6.0);
        let send_clicked = ui
            .add_sized([width, CHAT_CONTROL_HEIGHT], Button::new("Send"))
            .clicked();

        self.submit_on_enter_or_send(ctx, response, send_clicked);
    }

    fn render_roomy_chat_input(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            let control_gap = ui.spacing().item_spacing.x;
            let available_width =
                (ui.available_width() - SEND_WIDTH - control_gap).max(MIN_ROOMY_INPUT_WIDTH);
            let response = ui.add_sized(
                [available_width, CHAT_CONTROL_HEIGHT],
                TextEdit::singleline(&mut self.input)
                    .hint_text("Message Donna")
                    .desired_width(available_width),
            );
            let send_clicked = ui
                .add_sized([SEND_WIDTH, CHAT_CONTROL_HEIGHT], Button::new("Send"))
                .clicked();

            self.submit_on_enter_or_send(ctx, response, send_clicked);
        });
    }

    fn submit_on_enter_or_send(
        &mut self,
        ctx: &egui::Context,
        response: egui::Response,
        send_clicked: bool,
    ) {
        let enter_pressed = response.lost_focus()
            && ctx.input(|input| input.key_pressed(Key::Enter) && !input.modifiers.shift);

        if enter_pressed || send_clicked {
            self.submit_input(ctx);
        }
    }
}

fn chat_bar_text(text: impl Into<String>, color: egui::Color32) -> RichText {
    RichText::new(text)
        .font(FontId::proportional(13.0))
        .color(color)
}
