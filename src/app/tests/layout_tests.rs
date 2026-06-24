use super::super::chat_bar::chat_bar_reserved_height;
use super::super::layout::{
    AVATAR_ASPECT_RATIO, AVATAR_LAYOUT_SCALE, AVATAR_MAX_HEIGHT, CHAT_INNER_MARGIN,
    CHAT_WIDTH_RATIO, DEFAULT_WINDOW_SIZE, HORIZONTAL_GAP, MIN_WINDOW_SIZE, ShellLayout,
    avatar_image_size, shell_layout,
};
use super::super::native_options;
use super::{app_harness, app_harness_with_config, submit_text};
use crate::config::ModelConfig;
use eframe::App;
use eframe::egui::{self, Vec2};
use egui_kittest::kittest::{NodeT, Queryable};

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

    assert_eq!(options.viewport.app_id.as_deref(), Some("donna"));
    assert_eq!(
        options.viewport.inner_size,
        Some(DEFAULT_WINDOW_SIZE.into())
    );
    assert_eq!(
        options.viewport.min_inner_size,
        Some(MIN_WINDOW_SIZE.into())
    );
    assert_eq!(
        options.viewport.max_inner_size,
        Some(DEFAULT_WINDOW_SIZE.into())
    );
    assert_eq!(options.viewport.fullscreen, Some(false));
    assert_eq!(options.viewport.maximized, Some(false));
    assert_eq!(options.viewport.resizable, Some(false));
    assert_eq!(options.viewport.transparent, Some(true));
    assert_eq!(options.viewport.decorations, Some(false));
    assert_eq!(options.viewport.maximize_button, Some(false));
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
    let layout = shell_layout(MIN_WINDOW_SIZE.into());
    let chat_inner_size = chat_inner_size(layout);
    let reserved_bar_height = chat_bar_reserved_height(chat_inner_size.x, "", None);
    let minimum_scroll_height = 24.0;

    assert!(
        reserved_bar_height + minimum_scroll_height <= chat_inner_size.y + 0.5,
        "expected chat bar and scroll area to fit at native minimum; inner chat {chat_inner_size:?}, reserved {reserved_bar_height}"
    );
}

#[test]
fn minimum_chat_bar_input_fills_available_width() {
    let layout = shell_layout(MIN_WINDOW_SIZE.into());
    let expected_width = chat_inner_size(layout).x;
    let (_config_dir, mut harness) = app_harness(MIN_WINDOW_SIZE.into());
    harness.run_steps(1);

    let status = harness.get_by_label("Idle").rect();
    let model = harness.get_by_label("Ollama local").rect();
    let input = harness.get_by_role(egui::accesskit::Role::TextInput);
    let input_rect = input.rect();

    assert!(
        status.width() >= 18.0 && status.height() <= 24.0,
        "status label should stay horizontal, got {status:?}"
    );
    assert!(
        model.width() >= 64.0 && model.height() <= 24.0,
        "model label should stay horizontal, got {model:?}"
    );
    assert!(
        input_rect.width() >= expected_width - 1.0,
        "input should fill the chat bar width {expected_width}, got {input_rect:?}"
    );
    assert!(
        input.accesskit_node().is_focused(),
        "input should be focused on startup"
    );
}

#[test]
fn messages_do_not_expand_computed_chat_width() {
    let window_size = Vec2::new(820.0, 476.0);
    let layout = shell_layout(window_size);
    let max_message_width = (layout.chat_width - CHAT_INNER_MARGIN * 2.0).max(0.0) * 0.72;
    let (_config_dir, mut harness) = app_harness_with_config(window_size, |config| {
        config.ai.chat.selected_model = "mock-chat".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-chat".to_owned(),
            label: "Mock chat".to_owned(),
            provider: "mock".to_owned(),
            model: "mock".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });

    submit_text(&mut harness, "hi");
    harness.run_steps(8);

    let donna_reply = harness.get_by_label_contains("Mock response").rect();

    assert!(
        donna_reply.width() <= max_message_width + 1.0,
        "Donna reply should wrap inside computed chat width {max_message_width}, got {donna_reply:?}"
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
fn wider_avatar_sources_still_fit_inside_avatar_box() {
    let source = Vec2::new(1728.0, 2304.0);
    let bounds = Vec2::new(AVATAR_MAX_HEIGHT * AVATAR_ASPECT_RATIO, AVATAR_MAX_HEIGHT);
    let image_size = avatar_image_size(source, bounds);

    assert!(image_size.x <= bounds.x + 0.5);
    assert!(image_size.y <= bounds.y + 0.5);
    assert_close(image_size.x / image_size.y, source.x / source.y);
}
