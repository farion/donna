use super::super::chat_bar::chat_bar_reserved_height;
use super::super::layout::{
    AVATAR_ASPECT_RATIO, AVATAR_LAYOUT_SCALE, AVATAR_MAX_HEIGHT, CHAT_INNER_MARGIN,
    CHAT_WIDTH_RATIO, HORIZONTAL_GAP, ShellLayout, avatar_image_size, shell_layout,
};
use super::super::native_options;
use super::app_harness;
use eframe::App;
use eframe::egui::{self, Vec2};

fn shell_width(layout: ShellLayout) -> f32 {
    layout.avatar_width + layout.gap + layout.chat_width
}

fn shell_height(layout: ShellLayout) -> f32 {
    layout.avatar_height.max(layout.chat_height)
}

fn chat_inner_size(layout: ShellLayout) -> Vec2 {
    Vec2::new(
        (layout.chat_width - CHAT_INNER_MARGIN * 2.0).max(0.0),
        (layout.chat_height - CHAT_INNER_MARGIN * 2.0).max(0.0),
    )
}

fn assert_fits(actual: f32, available: f32) {
    assert!(
        actual <= available + 0.5,
        "expected {actual} to fit within {available}"
    );
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= 0.5,
        "expected {actual} to be close to {expected}"
    );
}

fn unscaled_avatar_height(available: Vec2) -> f32 {
    let available_width = available.x.max(0.0);
    let available_height = available.y.max(0.0);
    let width_bound = ((available_width - HORIZONTAL_GAP).max(0.0)
        / (AVATAR_ASPECT_RATIO + CHAT_WIDTH_RATIO))
        .max(0.0);

    available_height.min(width_bound).min(AVATAR_MAX_HEIGHT)
}

#[test]
fn native_window_is_transparent_and_undecorated() {
    let options = native_options();

    assert_eq!(options.viewport.transparent, Some(true));
    assert_eq!(options.viewport.decorations, Some(false));
    assert_eq!(options.viewport.has_shadow, Some(false));
    assert_eq!(options.viewport.title_shown, Some(false));
    assert_eq!(options.viewport.titlebar_shown, Some(false));
}

#[test]
fn app_clear_color_is_fully_transparent() {
    let (_config_dir, harness) = app_harness(Vec2::new(720.0, 480.0));
    let clear = harness.state().clear_color(&egui::Visuals::light());

    assert_eq!(clear[3], 0.0);
}

#[test]
fn roomy_shell_places_avatar_left_of_chat_with_default_ratio() {
    let available = Vec2::new(960.0, 640.0);
    let layout = shell_layout(available);
    let unscaled_height = unscaled_avatar_height(available);

    assert!(!layout.stacked);
    assert_close(layout.avatar_height, unscaled_height * AVATAR_LAYOUT_SCALE);
    assert_close(layout.chat_height, layout.avatar_height);
    assert_close(layout.chat_width, layout.avatar_height * CHAT_WIDTH_RATIO);
    assert_close(
        layout.avatar_width,
        layout.avatar_height * AVATAR_ASPECT_RATIO,
    );
    assert_fits(shell_width(layout), available.x);
    assert_fits(shell_height(layout), available.y);
}

#[test]
fn minimum_shell_keeps_chat_and_avatar_same_height() {
    let available = Vec2::new(720.0, 480.0);
    let layout = shell_layout(available);
    let unscaled_height = unscaled_avatar_height(available);

    assert!(!layout.stacked);
    assert_close(layout.avatar_height, unscaled_height * AVATAR_LAYOUT_SCALE);
    assert_close(layout.chat_height, layout.avatar_height);
    assert_close(layout.chat_width, layout.avatar_height * CHAT_WIDTH_RATIO);
    assert_close(layout.gap, HORIZONTAL_GAP);
    assert_fits(shell_width(layout), available.x);
    assert_fits(shell_height(layout), available.y);
}

#[test]
fn narrow_shell_shrinks_without_painting_extra_frames() {
    let available = Vec2::new(360.0, 360.0);
    let layout = shell_layout(available);
    let unscaled_height = unscaled_avatar_height(available);

    assert!(!layout.stacked);
    assert_close(layout.avatar_height, unscaled_height * AVATAR_LAYOUT_SCALE);
    assert_close(layout.chat_height, layout.avatar_height);
    assert_close(layout.chat_width, layout.avatar_height * CHAT_WIDTH_RATIO);
    assert_fits(shell_width(layout), available.x);
    assert_fits(shell_height(layout), available.y);
}

#[test]
fn compact_chat_bar_reserves_space_inside_minimum_chat_fill() {
    let layout = shell_layout(Vec2::new(720.0, 480.0));
    let chat_inner_size = chat_inner_size(layout);
    let reserved_bar_height = chat_bar_reserved_height(chat_inner_size.x);
    let minimum_scroll_height = 24.0;

    assert!(
        reserved_bar_height + minimum_scroll_height <= chat_inner_size.y + 0.5,
        "expected chat bar and scroll area to fit at native minimum; inner chat {chat_inner_size:?}, reserved {reserved_bar_height}"
    );
}

#[test]
fn avatar_image_fit_uses_full_avatar_height_and_preserves_ratio() {
    let source = Vec2::new(1141.0, 2183.0);
    let bounds = Vec2::new(AVATAR_MAX_HEIGHT * AVATAR_ASPECT_RATIO, AVATAR_MAX_HEIGHT);
    let image_size = avatar_image_size(source, bounds);

    assert!(image_size.x <= bounds.x + 0.5);
    assert!(image_size.y <= bounds.y + 0.5);
    assert_close(image_size.x / image_size.y, source.x / source.y);
    assert!(image_size.x < image_size.y);
}

#[test]
fn wider_avatar_sources_still_fit_inside_half_scale_box() {
    let source = Vec2::new(1728.0, 2304.0);
    let bounds = Vec2::new(AVATAR_MAX_HEIGHT * AVATAR_ASPECT_RATIO, AVATAR_MAX_HEIGHT);
    let image_size = avatar_image_size(source, bounds);

    assert!(image_size.x <= bounds.x + 0.5);
    assert!(image_size.y <= bounds.y + 0.5);
    assert_close(image_size.x / image_size.y, source.x / source.y);
}
