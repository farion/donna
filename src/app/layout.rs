use eframe::egui::Vec2;

pub(super) const AVATAR_MAX_HEIGHT: f32 = 900.0;
pub(super) const AVATAR_ASPECT_RATIO: f32 = 0.52;
pub(super) const AVATAR_LAYOUT_SCALE: f32 = 1.0;
pub(super) const CHAT_WIDTH_RATIO: f32 = 0.8;
pub(super) const HORIZONTAL_GAP: f32 = 20.0;
pub(super) const CHAT_INNER_MARGIN: f32 = 14.0;
pub(super) const DEFAULT_WINDOW_SIZE: [f32; 2] = [614.0, 450.0];
pub(super) const MIN_WINDOW_SIZE: [f32; 2] = [337.0, 240.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ShellLayout {
    pub avatar_width: f32,
    pub avatar_height: f32,
    pub chat_width: f32,
    pub chat_height: f32,
    pub gap: f32,
    pub stacked: bool,
}

pub(super) fn shell_layout(available: Vec2) -> ShellLayout {
    let available_width = available.x.max(0.0);
    let available_height = available.y.max(0.0);
    let width_bound = ((available_width - HORIZONTAL_GAP).max(0.0)
        / (AVATAR_ASPECT_RATIO + CHAT_WIDTH_RATIO))
        .max(0.0);
    let fitted_avatar_height = available_height
        .min(width_bound)
        .clamp(0.0, AVATAR_MAX_HEIGHT);
    let avatar_height = fitted_avatar_height * AVATAR_LAYOUT_SCALE;

    ShellLayout {
        avatar_width: avatar_height * AVATAR_ASPECT_RATIO,
        avatar_height,
        chat_width: avatar_height * CHAT_WIDTH_RATIO,
        chat_height: avatar_height,
        gap: HORIZONTAL_GAP,
        stacked: false,
    }
}

pub(super) fn avatar_image_size(source_size: Vec2, bounds: Vec2) -> Vec2 {
    if source_size.x <= 0.0 || source_size.y <= 0.0 || bounds.x <= 0.0 || bounds.y <= 0.0 {
        return Vec2::ZERO;
    }

    let scale = (bounds.x / source_size.x).min(bounds.y / source_size.y);
    source_size * scale
}
