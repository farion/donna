use super::DonnaApp;
use crate::command::command_suggestions;
use eframe::egui::{self, Button, Color32, FontId, RichText};

impl DonnaApp {
    pub(super) fn render_command_suggestions(&mut self, ui: &mut egui::Ui) {
        let suggestions = command_suggestions(&self.input);
        if suggestions.is_empty() {
            return;
        }

        ui.horizontal_wrapped(|ui| {
            for suggestion in suggestions {
                let response = ui
                    .add_sized([112.0, 28.0], Button::new(suggestion.command))
                    .on_hover_text(suggestion.summary);
                if response.clicked() {
                    self.input = match suggestion.command {
                        "/changechar" | "/theme" => format!("{} ", suggestion.command),
                        _ => suggestion.command.to_owned(),
                    };
                    self.input_notice = None;
                    self.input_error = None;
                }
            }
        });
        ui.add_space(4.0);
    }

    pub(super) fn render_input_feedback(&self, ui: &mut egui::Ui) {
        if let Some(error) = &self.input_error {
            ui.label(
                RichText::new(error)
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(154, 56, 48)),
            );
        } else if let Some(notice) = &self.input_notice {
            ui.label(
                RichText::new(notice)
                    .font(FontId::proportional(12.0))
                    .color(Color32::from_rgb(72, 83, 92)),
            );
        }
    }
}
