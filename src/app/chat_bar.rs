use super::ui_style::palette_for;
use super::{DonnaApp, status};
use eframe::egui::{self, Align, FontId, Key, Label, Layout, Margin, RichText, TextEdit};

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
        self.submit_on_enter(ctx, response);
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
