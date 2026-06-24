use crate::chat::Speaker;
use crate::config::{AppConfig, UiThemeMode};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText, Vec2,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct UiPalette {
    pub shell_fill: Color32,
    pub avatar_fill: Color32,
    pub chat_fill: Color32,
    pub heading_text: Color32,
    pub muted_text: Color32,
    pub notice_text: Color32,
    pub warning_text: Color32,
    pub error_text: Color32,
}

pub(super) fn apply_style(ctx: &egui::Context, theme: UiThemeMode) {
    ctx.set_visuals_of(egui::Theme::Light, app_visuals(egui::Theme::Light));
    ctx.set_visuals_of(egui::Theme::Dark, app_visuals(egui::Theme::Dark));
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = Vec2::new(10.0, 8.0);
        style.spacing.button_padding = Vec2::new(12.0, 8.0);
        style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
        style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
        style.visuals.widgets.active.corner_radius = CornerRadius::same(6);
    });
    ctx.set_theme(theme_preference(theme));
}

pub(super) fn palette_for(theme: egui::Theme) -> UiPalette {
    match theme {
        egui::Theme::Light => UiPalette {
            shell_fill: Color32::from_rgb(242, 244, 239),
            avatar_fill: Color32::from_rgb(232, 236, 230),
            chat_fill: Color32::from_rgb(252, 252, 249),
            heading_text: Color32::from_rgb(26, 33, 38),
            muted_text: Color32::from_rgb(89, 96, 103),
            notice_text: Color32::from_rgb(72, 83, 92),
            warning_text: Color32::from_rgb(148, 75, 45),
            error_text: Color32::from_rgb(154, 56, 48),
        },
        egui::Theme::Dark => UiPalette {
            shell_fill: Color32::from_rgb(20, 23, 25),
            avatar_fill: Color32::from_rgb(38, 44, 48),
            chat_fill: Color32::from_rgb(28, 32, 36),
            heading_text: Color32::from_rgb(238, 243, 238),
            muted_text: Color32::from_rgb(172, 183, 179),
            notice_text: Color32::from_rgb(188, 198, 194),
            warning_text: Color32::from_rgb(239, 170, 111),
            error_text: Color32::from_rgb(250, 139, 130),
        },
    }
}

pub(super) fn render_message(
    ui: &mut egui::Ui,
    speaker: Speaker,
    text: &str,
    chat_width: f32,
    config: &AppConfig,
) {
    let theme = ui.ctx().theme();
    let color = match speaker {
        Speaker::Donna => parse_hex_color(&config.ui.donna_message_color)
            .unwrap_or_else(|| default_message_color(speaker, theme)),
        Speaker::User => parse_hex_color(&config.ui.user_message_color)
            .unwrap_or_else(|| default_message_color(speaker, theme)),
    };
    let text_color = readable_text_color(color);
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
                ui.label(
                    RichText::new(text)
                        .font(FontId::proportional(14.0))
                        .color(text_color),
                );
            });
    });
}

fn theme_preference(theme: UiThemeMode) -> egui::ThemePreference {
    match theme {
        UiThemeMode::Auto => egui::ThemePreference::System,
        UiThemeMode::Light => egui::ThemePreference::Light,
        UiThemeMode::Dark => egui::ThemePreference::Dark,
    }
}

fn app_visuals(theme: egui::Theme) -> egui::Visuals {
    let mut visuals = match theme {
        egui::Theme::Light => egui::Visuals::light(),
        egui::Theme::Dark => egui::Visuals::dark(),
    };
    let palette = palette_for(theme);

    visuals.panel_fill = palette.shell_fill;
    visuals.window_fill = palette.chat_fill;
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(6);
    visuals.warn_fg_color = palette.warning_text;
    visuals.error_fg_color = palette.error_text;
    visuals.extreme_bg_color = match theme {
        egui::Theme::Light => Color32::from_rgb(247, 249, 244),
        egui::Theme::Dark => Color32::from_rgb(18, 21, 23),
    };
    visuals.text_edit_bg_color = Some(match theme {
        egui::Theme::Light => Color32::from_rgb(255, 255, 252),
        egui::Theme::Dark => Color32::from_rgb(34, 39, 43),
    });
    visuals.selection.bg_fill = match theme {
        egui::Theme::Light => Color32::from_rgb(105, 137, 185),
        egui::Theme::Dark => Color32::from_rgb(88, 121, 173),
    };

    visuals
}

fn default_message_color(speaker: Speaker, theme: egui::Theme) -> Color32 {
    match (speaker, theme) {
        (Speaker::Donna, egui::Theme::Light) => Color32::from_rgb(238, 245, 255),
        (Speaker::User, egui::Theme::Light) => Color32::from_rgb(234, 247, 239),
        (Speaker::Donna, egui::Theme::Dark) => Color32::from_rgb(42, 54, 72),
        (Speaker::User, egui::Theme::Dark) => Color32::from_rgb(35, 68, 55),
    }
}

fn readable_text_color(background: Color32) -> Color32 {
    let luminance = (0.2126 * f32::from(background.r())
        + 0.7152 * f32::from(background.g())
        + 0.0722 * f32::from(background.b()))
        / 255.0;

    if luminance >= 0.55 {
        Color32::from_rgb(28, 35, 40)
    } else {
        Color32::from_rgb(242, 247, 243)
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
