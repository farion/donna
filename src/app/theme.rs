use super::{DonnaApp, ui_style::apply_style};
use crate::config::UiThemeMode;

impl DonnaApp {
    pub(super) fn handle_theme_command(&mut self, mode: Option<&str>, ctx: &eframe::egui::Context) {
        self.pending_exit_confirmation = false;
        self.state = super::DonnaState::Command;

        let Some(theme) = mode.and_then(UiThemeMode::parse) else {
            self.show_command_error("Usage: /theme auto|light|dark");
            return;
        };

        let previous_theme = self.config.ui.theme;
        self.config.ui.theme = theme;

        match self.config.save_to_path(&self.config_path) {
            Ok(()) => {
                apply_style(ctx, theme);
                ctx.request_repaint();
                self.input_notice = Some(format!("Theme set to {}.", theme.as_str()));
            }
            Err(error) => {
                self.config.ui.theme = previous_theme;
                self.config_notice = Some(error.to_string());
            }
        }
    }
}
