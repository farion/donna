use crate::chat::Speaker;
use crate::config::{AppConfig, UiThemeMode};
use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText, Vec2,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct UiPalette {
    pub chat_fill: Color32,
    pub notice_text: Color32,
    pub warning_text: Color32,
    pub error_text: Color32,
    pub attention_card_fill: Color32,
    pub attention_card_stroke: Color32,
    pub attention_level_text: Color32,
    pub attention_title_text: Color32,
    pub attention_body_text: Color32,
    pub review_heading_text: Color32,
    pub review_card_fill: Color32,
    pub review_card_stroke: Color32,
    pub review_label_text: Color32,
    pub review_metadata_text: Color32,
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
        disable_debug_overlays(style);
    });
    ctx.set_theme(theme_preference(theme));
}

fn disable_debug_overlays(_style: &mut egui::Style) {
    #[cfg(debug_assertions)]
    {
        _style.debug.debug_on_hover = false;
        _style.debug.debug_on_hover_with_all_modifiers = false;
        _style.debug.hover_shows_next = false;
        _style.debug.show_expand_width = false;
        _style.debug.show_expand_height = false;
        _style.debug.show_resize = false;
        _style.debug.show_interactive_widgets = false;
        _style.debug.show_widget_hits = false;
        _style.debug.warn_if_rect_changes_id = false;
        _style.debug.show_unaligned = false;
        _style.debug.show_focused_widget = false;
    }
}

pub(super) fn palette_for(theme: egui::Theme) -> UiPalette {
    match theme {
        egui::Theme::Light => UiPalette {
            chat_fill: Color32::from_rgb(252, 252, 249),
            notice_text: Color32::from_rgb(72, 83, 92),
            warning_text: Color32::from_rgb(148, 75, 45),
            error_text: Color32::from_rgb(154, 56, 48),
            attention_card_fill: Color32::from_rgb(255, 248, 224),
            attention_card_stroke: Color32::from_rgb(232, 211, 171),
            attention_level_text: Color32::from_rgb(127, 83, 26),
            attention_title_text: Color32::from_rgb(26, 33, 38),
            attention_body_text: Color32::from_rgb(72, 83, 92),
            review_heading_text: Color32::from_rgb(78, 64, 54),
            review_card_fill: Color32::from_rgb(250, 245, 238),
            review_card_stroke: Color32::from_rgb(223, 207, 189),
            review_label_text: Color32::from_rgb(117, 81, 55),
            review_metadata_text: Color32::from_rgb(89, 96, 103),
        },
        egui::Theme::Dark => UiPalette {
            chat_fill: Color32::from_rgb(28, 32, 36),
            notice_text: Color32::from_rgb(188, 198, 194),
            warning_text: Color32::from_rgb(239, 170, 111),
            error_text: Color32::from_rgb(250, 139, 130),
            attention_card_fill: Color32::from_rgb(48, 41, 32),
            attention_card_stroke: Color32::from_rgb(104, 80, 48),
            attention_level_text: Color32::from_rgb(239, 170, 111),
            attention_title_text: Color32::from_rgb(238, 243, 238),
            attention_body_text: Color32::from_rgb(188, 198, 194),
            review_heading_text: Color32::from_rgb(238, 243, 238),
            review_card_fill: Color32::from_rgb(40, 37, 34),
            review_card_stroke: Color32::from_rgb(84, 76, 66),
            review_label_text: Color32::from_rgb(239, 170, 111),
            review_metadata_text: Color32::from_rgb(188, 198, 194),
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
                ui.add(
                    egui::Label::new(
                        RichText::new(text)
                            .font(FontId::proportional(14.0))
                            .color(text_color),
                    )
                    .wrap(),
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

pub(super) fn app_visuals(theme: egui::Theme) -> egui::Visuals {
    let mut visuals = match theme {
        egui::Theme::Light => egui::Visuals::light(),
        egui::Theme::Dark => egui::Visuals::dark(),
    };
    let palette = palette_for(theme);

    visuals.panel_fill = Color32::TRANSPARENT;
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
    let quiet_stroke = egui::Stroke::new(0.0, Color32::TRANSPARENT);
    visuals.widgets.inactive.bg_stroke = quiet_stroke;
    visuals.widgets.hovered.bg_stroke = quiet_stroke;
    visuals.widgets.active.bg_stroke = quiet_stroke;
    visuals.widgets.open.bg_stroke = quiet_stroke;
    visuals.widgets.noninteractive.bg_stroke = quiet_stroke;

    visuals
}

pub(super) fn default_message_color(speaker: Speaker, theme: egui::Theme) -> Color32 {
    match (speaker, theme) {
        (Speaker::Donna, egui::Theme::Light) => Color32::from_rgb(238, 245, 255),
        (Speaker::User, egui::Theme::Light) => Color32::from_rgb(234, 247, 239),
        (Speaker::Donna, egui::Theme::Dark) => Color32::from_rgb(42, 54, 72),
        (Speaker::User, egui::Theme::Dark) => Color32::from_rgb(35, 68, 55),
    }
}

pub(super) fn readable_text_color(background: Color32) -> Color32 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_feedback_and_cards_keep_normal_text_contrast() {
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let palette = palette_for(theme);

            assert_aa(palette.notice_text, palette.chat_fill, "command notice");
            assert_aa(palette.error_text, palette.chat_fill, "command error");
            assert_aa(
                palette.review_heading_text,
                palette.chat_fill,
                "review heading",
            );
            assert_aa(
                palette.attention_level_text,
                palette.attention_card_fill,
                "attention level",
            );
            assert_aa(
                palette.attention_title_text,
                palette.attention_card_fill,
                "attention title",
            );
            assert_aa(
                palette.attention_body_text,
                palette.attention_card_fill,
                "attention body",
            );
            assert_aa(
                palette.review_label_text,
                palette.review_card_fill,
                "review label",
            );
            assert_aa(
                palette.review_metadata_text,
                palette.review_card_fill,
                "review metadata",
            );
        }
    }

    fn assert_aa(foreground: Color32, background: Color32, label: &str) {
        let ratio = contrast_ratio(foreground, background);
        assert!(
            ratio >= 4.5,
            "{label} contrast {ratio:.2}:1 is below WCAG AA normal text"
        );
    }

    fn contrast_ratio(foreground: Color32, background: Color32) -> f32 {
        let foreground = relative_luminance(foreground);
        let background = relative_luminance(background);
        let lighter = foreground.max(background);
        let darker = foreground.min(background);

        (lighter + 0.05) / (darker + 0.05)
    }

    fn relative_luminance(color: Color32) -> f32 {
        let channel = |value: u8| {
            let value = f32::from(value) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };

        0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
    }
}
