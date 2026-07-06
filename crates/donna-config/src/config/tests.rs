use super::{AppConfig, UiThemeMode};

#[test]
fn default_config_has_non_secret_model_references() {
    let config = AppConfig::default();

    assert_eq!(config.ui.theme, UiThemeMode::Auto);
    assert_eq!(config.avatar.character, "donna");
    assert_eq!(config.ai.chat.selected_model, "ollama-local");
    assert!(
        config
            .ai
            .models
            .iter()
            .any(|model| model.secret_ref.is_none())
    );
    assert!(
        config
            .ai
            .models
            .iter()
            .any(|model| model.secret_ref.as_deref() == Some("donna/openai"))
    );
}

#[test]
fn creates_and_reloads_default_config() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("donna.toml");

    let mut config = AppConfig::load_or_create_at(&path).expect("create config");
    config.ui.theme = UiThemeMode::Dark;
    config.ai.chat.selected_model = "openai-compatible".to_owned();
    config.data.database_path = dir.path().join("state.sqlite3");
    config.save_to_path(&path).expect("save config");

    let loaded = AppConfig::load_at(path).expect("reload config");
    assert_eq!(loaded.ui.theme, UiThemeMode::Dark);
    assert_eq!(loaded.ai.chat.selected_model, "openai-compatible");
    assert_eq!(loaded.data.database_path, dir.path().join("state.sqlite3"));
    assert!(loaded.offline.show_stale_data_warnings);
}

#[test]
fn serialized_default_config_keeps_auto_theme_under_ui() {
    let contents = toml::to_string_pretty(&AppConfig::default()).expect("serialize config");

    assert!(contents.contains("[ui]\ntheme = \"auto\""));
}

#[test]
fn missing_ui_theme_defaults_to_auto() {
    let contents = "\
[ui]
donna_message_color = \"#eef5ff\"
user_message_color = \"#eaf7ef\"
";
    let config = toml::from_str::<AppConfig>(contents).expect("legacy config");

    assert_eq!(config.ui.theme, UiThemeMode::Auto);
}

#[test]
fn invalid_ui_theme_is_rejected() {
    let contents = "\
[ui]
theme = \"midnight\"
";

    assert!(toml::from_str::<AppConfig>(contents).is_err());
}

#[test]
fn invalid_config_falls_back_with_error_message() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "[ai").expect("write bad toml");

    let (config, error) = AppConfig::load_or_default_at(path);
    assert_eq!(config.ai.chat.selected_model, "ollama-local");
    assert!(error.expect("error").contains("invalid config TOML"));
}

#[test]
fn serialized_config_keeps_secret_values_out_of_toml() {
    let mut config = AppConfig::default();
    config.microsoft.client_id = Some("client-id".to_owned());
    config.microsoft.token_secret_ref = Some("donna/microsoft".to_owned());

    let contents = toml::to_string_pretty(&config).expect("serialize config");

    assert!(contents.contains("secret_ref = \"donna/openai\""));
    assert!(contents.contains("client_id = \"client-id\""));
    assert!(contents.contains("token_secret_ref = \"donna/microsoft\""));
    assert!(!contents.contains("api_key"));
    assert!(!contents.contains("access_token"));
    assert!(!contents.contains("refresh_token"));
    assert!(!contents.contains("password"));
}
