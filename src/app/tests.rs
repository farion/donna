use super::*;
use crate::config::UiThemeMode;
use crate::storage::{NewAttentionItem, SearchQuery};
use eframe::egui::{self, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

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

#[test]
fn exit_command_requires_confirmation_before_stopping_task_runner() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    submit_text(&mut harness, "/exit");

    assert!(harness.state().task_runner_state.is_running());
    assert_eq!(harness.state().state_label(), "Command");
    assert_eq!(
        harness.state().input_notice.as_deref(),
        Some("Type /exit confirm to stop Donna and background tasks.")
    );

    submit_text(&mut harness, "/exit confirm");

    assert!(!harness.state().task_runner_state.is_running());
}

#[test]
fn commands_do_not_enter_chat_timeline_and_unknown_commands_are_inline_errors() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let initial_count = harness.state().chat.messages().len();

    submit_text(&mut harness, "/dance");

    assert_eq!(harness.state().chat.messages().len(), initial_count);
    assert_eq!(
        harness.state().input_error.as_deref(),
        Some("Unknown command: /dance")
    );

    submit_text(&mut harness, "/hide");

    assert_eq!(harness.state().chat.messages().len(), initial_count);
}

#[test]
fn changechar_persists_embedded_avatar_character() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let config_path = harness.state().config_path.clone();
    {
        let state = harness.state_mut();
        state.config.avatar.character = "missing".to_owned();
        state
            .config
            .save_to_path(&state.config_path)
            .expect("save changed config");
    }

    submit_text(&mut harness, "/changechar donna");

    let config = crate::config::AppConfig::load_at(config_path).expect("reload config");
    assert_eq!(harness.state().config.avatar.character, "donna");
    assert_eq!(config.avatar.character, "donna");
    assert_eq!(
        harness.state().input_notice.as_deref(),
        Some("Avatar changed to donna.")
    );
}

#[test]
fn theme_command_updates_context_and_persists_without_chat_messages() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let config_path = harness.state().config_path.clone();
    let initial_count = harness.state().chat.messages().len();

    for (input, mode, preference, notice) in [
        (
            "/theme light",
            UiThemeMode::Light,
            egui::ThemePreference::Light,
            "Theme set to light.",
        ),
        (
            "/theme dark",
            UiThemeMode::Dark,
            egui::ThemePreference::Dark,
            "Theme set to dark.",
        ),
        (
            "/theme AUTO",
            UiThemeMode::Auto,
            egui::ThemePreference::System,
            "Theme set to auto.",
        ),
    ] {
        submit_text(&mut harness, input);

        let persisted = crate::config::AppConfig::load_at(&config_path).expect("reload config");
        assert_eq!(harness.state().config.ui.theme, mode);
        assert_eq!(persisted.ui.theme, mode);
        assert_eq!(
            harness.ctx.options(|options| options.theme_preference),
            preference
        );
        assert_eq!(harness.state().input_notice.as_deref(), Some(notice));
        assert_eq!(harness.state().chat.messages().len(), initial_count);
    }
}

#[test]
fn invalid_theme_command_shows_inline_error_without_persisting() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ui.theme = UiThemeMode::Light;
    });
    let config_path = harness.state().config_path.clone();
    let initial_count = harness.state().chat.messages().len();

    submit_text(&mut harness, "/theme midnight");

    let persisted = crate::config::AppConfig::load_at(&config_path).expect("reload config");
    assert_eq!(harness.state().config.ui.theme, UiThemeMode::Light);
    assert_eq!(persisted.ui.theme, UiThemeMode::Light);
    assert_eq!(
        harness.ctx.options(|options| options.theme_preference),
        egui::ThemePreference::Light
    );
    assert_eq!(
        harness.state().input_error.as_deref(),
        Some("Usage: /theme auto|light|dark")
    );
    assert_eq!(harness.state().chat.messages().len(), initial_count);

    submit_text(&mut harness, "/theme");

    let persisted = crate::config::AppConfig::load_at(config_path).expect("reload config");
    assert_eq!(harness.state().config.ui.theme, UiThemeMode::Light);
    assert_eq!(persisted.ui.theme, UiThemeMode::Light);
    assert_eq!(
        harness.state().input_error.as_deref(),
        Some("Usage: /theme auto|light|dark")
    );
    assert_eq!(harness.state().chat.messages().len(), initial_count);
}

#[test]
fn command_mode_renders_compact_suggestion_row() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    harness.state_mut().input = "/".to_owned();
    harness.run_steps(1);

    assert!(harness.query_by_label("/hide").is_some());
    assert!(harness.query_by_label("/exit").is_some());
    assert!(harness.query_by_label("/changechar").is_some());
    assert!(harness.query_by_label("/theme").is_some());
    assert_eq!(harness.state().state_label(), "Command");
}

#[test]
fn attention_card_renders_controls_and_can_dismiss_active_item() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let item_id = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_attention_item(&NewAttentionItem {
            source_type: "follow_up".to_owned(),
            source_id: Some(7),
            level: "important".to_owned(),
            title: "Anna is waiting".to_owned(),
            body: Some("Reply about billing retries".to_owned()),
            due_at: None,
            payload: None,
        })
        .expect("attention")
        .id;

    harness.run_steps(1);

    assert_eq!(harness.state().state_label(), "Attention");
    assert!(harness.query_by_label("Anna is waiting").is_some());
    assert!(harness.query_by_label("Done").is_some());
    assert!(harness.query_by_label("Snooze 1h").is_some());
    assert!(harness.query_by_label("Dismiss").is_some());
    assert!(harness.query_by_label("Not important").is_some());

    let state = harness.state_mut();
    assert!(
        state
            .attention
            .dismiss_active(state.store.as_ref(), &mut state.config_notice)
    );

    let item = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .attention_item(item_id)
        .expect("attention item");
    assert_eq!(item.status, "dismissed");
}

#[test]
fn sensitive_memory_review_requires_approval_then_allows_correction_and_forget() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let raw = "remember that my password is swordfish; keep this whole raw line private";

    submit_text(&mut harness, raw);

    let review_id = harness
        .state()
        .sensitive_memory_reviews
        .first_pending_id()
        .expect("pending sensitive memory");
    assert_eq!(harness.state().sensitive_memory_reviews.pending_count(), 1);
    assert!(
        harness
            .state()
            .store
            .as_ref()
            .expect("store")
            .search(&SearchQuery::text("swordfish"))
            .expect("search before approval")
            .is_empty()
    );

    assert!(harness.state_mut().correct_pending_sensitive_memory(
        review_id,
        "Fact: credential reference lives in the private vault"
    ));
    let memory_id = harness
        .state_mut()
        .approve_pending_sensitive_memory(review_id)
        .expect("approve pending")
        .expect("saved memory id");

    let store = harness.state().store.as_ref().expect("store");
    let stored = store.memory(memory_id).expect("stored memory");
    assert_eq!(
        stored.content,
        "Fact: credential reference lives in the private vault"
    );
    assert_ne!(stored.content, raw);
    assert!(
        store
            .search(&SearchQuery::text("swordfish"))
            .expect("search sensitive raw token")
            .is_empty()
    );
    assert_eq!(
        store
            .search(&SearchQuery::text("credential"))
            .expect("search corrected memory")[0]
            .record_id,
        memory_id
    );

    assert!(harness.state_mut().correct_reviewed_sensitive_memory(
        memory_id,
        "Fact: private vault holds the credential reference"
    ));
    harness
        .state_mut()
        .update_reviewed_sensitive_memory(memory_id)
        .expect("update reviewed");
    let store = harness.state().store.as_ref().expect("store");
    assert_eq!(
        store.memory(memory_id).expect("updated memory").content,
        "Fact: private vault holds the credential reference"
    );
    assert!(
        store
            .search(&SearchQuery::text("lives"))
            .expect("search old correction")
            .is_empty()
    );

    harness
        .state_mut()
        .forget_reviewed_sensitive_memory(memory_id)
        .expect("forget reviewed");
    let store = harness.state().store.as_ref().expect("store");
    assert!(
        store
            .memory(memory_id)
            .expect("forgotten memory")
            .forgotten_at
            .is_some()
    );
    assert!(
        store
            .search(&SearchQuery::text("credential"))
            .expect("search forgotten")
            .is_empty()
    );
}

#[test]
fn deleting_pending_sensitive_memory_keeps_it_out_of_storage() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    submit_text(&mut harness, "remember that my salary target is private");

    let review_id = harness
        .state()
        .sensitive_memory_reviews
        .first_pending_id()
        .expect("pending sensitive memory");

    assert!(
        harness
            .state_mut()
            .delete_pending_sensitive_memory(review_id)
    );
    assert_eq!(harness.state().sensitive_memory_reviews.pending_count(), 0);
    assert!(
        harness
            .state()
            .store
            .as_ref()
            .expect("store")
            .search(&SearchQuery::text("salary"))
            .expect("search deleted pending")
            .is_empty()
    );
}

fn submit_text(harness: &mut Harness<'static, DonnaApp>, text: &str) {
    let ctx = harness.ctx.clone();
    harness.state_mut().input = text.to_owned();
    harness.state_mut().submit_input(&ctx);
}

fn app_harness(size: Vec2) -> (tempfile::TempDir, Harness<'static, DonnaApp>) {
    app_harness_with_config(size, |_| {})
}

fn app_harness_with_config(
    size: Vec2,
    configure: impl FnOnce(&mut crate::config::AppConfig),
) -> (tempfile::TempDir, Harness<'static, DonnaApp>) {
    let config_dir = tempfile::tempdir().expect("temp config dir");
    let config_path = config_dir.path().join("donna.toml");
    let mut config = crate::config::AppConfig::default();
    config.data.database_path = config_dir.path().join("donna.sqlite3");
    configure(&mut config);
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

mod layout_tests;
mod visual_evidence;
