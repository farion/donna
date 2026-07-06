use super::DonnaApp;
use super::ui_style::palette_for;
use eframe::egui::{self, FontId, RichText};

impl DonnaApp {
    pub(super) fn render_input_feedback(&self, ui: &mut egui::Ui) {
        let palette = palette_for(ui.ctx().theme());
        if let Some(error) = &self.input_error {
            ui.label(
                RichText::new(error)
                    .font(FontId::proportional(12.0))
                    .color(palette.error_text),
            );
        } else if let Some(notice) = &self.input_notice {
            ui.label(
                RichText::new(notice)
                    .font(FontId::proportional(12.0))
                    .color(palette.notice_text),
            );
        }
    }
}
