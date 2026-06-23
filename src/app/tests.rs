use super::*;
use eframe::egui::{self, Vec2};
use egui_kittest::Harness;

fn shell_width(layout: ShellLayout) -> f32 {
    layout.avatar_side + layout.chat_width + layout.gap + SHELL_FRAME_MARGIN
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

#[test]
fn roomy_shell_preserves_default_chat_ratio() {
    let available = Vec2::new(960.0, 640.0);
    let layout = shell_layout(available);

    assert!(!layout.stacked);
    assert_close(layout.avatar_side, AVATAR_MAX_SIDE);
    assert_close(layout.chat_width, AVATAR_MAX_SIDE * CHAT_WIDTH_RATIO);
    assert_fits(shell_width(layout), available.x);
}

#[test]
fn minimum_shell_shrinks_row_before_overflowing() {
    let available = Vec2::new(720.0, 480.0);
    let layout = shell_layout(available);

    assert!(!layout.stacked);
    assert!(layout.avatar_side < available.y);
    assert!(layout.chat_width >= CHAT_MIN_WIDTH);
    assert_close(layout.chat_width, layout.avatar_side * CHAT_WIDTH_RATIO);
    assert_fits(shell_width(layout), available.x);
}

#[test]
fn narrow_shell_uses_single_column_before_chat_gets_too_small() {
    let available = Vec2::new(560.0, 480.0);
    let layout = shell_layout(available);

    assert!(layout.stacked);
    assert_fits(layout.chat_width + CHAT_FRAME_MARGIN, available.x);
}

#[test]
fn hide_command_requests_window_minimize() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    submit_text(&mut harness, "/hide");
    harness.run_steps(1);

    let commands = &harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .expect("root viewport output")
        .commands;

    assert_eq!(harness.state().state_label(), "Hidden");
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Minimized(true))),
        "expected /hide to emit ViewportCommand::Minimized(true), got {commands:?}"
    );
}

fn submit_text(harness: &mut Harness<'static, DonnaApp>, text: &str) {
    let ctx = harness.ctx.clone();
    harness.state_mut().input = text.to_owned();
    harness.state_mut().submit_input(&ctx);
}

fn app_harness(size: Vec2) -> (tempfile::TempDir, Harness<'static, DonnaApp>) {
    let config_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = config_dir.path().join("donna.toml");
    let mut config = crate::config::AppConfig::default();
    config.data.database_path = config_dir.path().join("donna.sqlite3");
    config
        .save_to_path(&config_path)
        .expect("write test config");
    let harness = Harness::<DonnaApp>::builder()
        .with_size(size)
        .with_theme(egui::Theme::Light)
        .with_max_steps(8)
        .build_eframe(move |creation| {
            DonnaApp::new_with_config_path(creation, config_path.clone())
        });

    (config_dir, harness)
}

mod visual_evidence;
