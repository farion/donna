use eframe::egui::Vec2;

pub(super) const AVATAR_MIN_SIDE: f32 = 220.0;
pub(super) const AVATAR_MAX_SIDE: f32 = 480.0;
pub(super) const CHAT_WIDTH_RATIO: f32 = 0.8;
pub(super) const CHAT_MIN_WIDTH: f32 = 280.0;
pub(super) const ROW_GAP: f32 = 24.0;
pub(super) const COMPACT_ROW_GAP: f32 = 16.0;
pub(super) const AVATAR_FRAME_MARGIN: f32 = 32.0;
pub(super) const CHAT_FRAME_MARGIN: f32 = 28.0;
pub(super) const SHELL_FRAME_MARGIN: f32 = AVATAR_FRAME_MARGIN + CHAT_FRAME_MARGIN;
// Half of the former 82% square image fill, now used as an aspect-fit bound.
pub(super) const AVATAR_IMAGE_SCALE: f32 = 0.41;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ShellLayout {
    pub avatar_side: f32,
    pub chat_width: f32,
    pub chat_height: f32,
    pub gap: f32,
    pub stacked: bool,
}

pub(super) fn shell_layout(available: Vec2) -> ShellLayout {
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

pub(super) fn avatar_image_size(source_size: Vec2, avatar_side: f32) -> Vec2 {
    let max_size = Vec2::splat(avatar_side * AVATAR_IMAGE_SCALE);

    if source_size.x <= 0.0 || source_size.y <= 0.0 || max_size.x <= 0.0 || max_size.y <= 0.0 {
        return Vec2::ZERO;
    }

    let scale = (max_size.x / source_size.x).min(max_size.y / source_size.y);
    source_size * scale
}
