use super::*;
use crate::config::{ModelConfig, UiThemeMode};
use crate::storage::{NewAttentionItem, NewTodo, SearchQuery, StoredMemory};
use eframe::egui::{self, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

#[test]
fn hide_command_hides_window_without_stopping_app() {
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
    assert!(harness.state().task_runner_state.is_running());
    assert!(!harness.state().hide_requested.load(Ordering::SeqCst));
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Close)),
        "/hide must keep the same app/session alive, got {commands:?}"
    );
}

#[test]
fn hide_command_works_while_thinking() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    harness.state_mut().response_in_progress = true;
    harness.state_mut().response_started_at = Some(Instant::now());

    submit_text(&mut harness, "/hide");
    harness.run_steps(1);

    assert_eq!(harness.state().state_label(), "Hidden");
    assert_ne!(
        harness.state().input_notice.as_deref(),
        Some("Donna is still thinking.")
    );
    assert!(harness.state().task_runner_state.is_running());
}

#[test]
fn wakeup_event_brings_ui_to_foreground() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let (sender, receiver) = std::sync::mpsc::channel();
    harness.state_mut().wakeup_receiver = Some(Arc::new(Mutex::new(receiver)));
    harness.state_mut().state = DonnaState::Hidden;

    sender.send(IpcEvent::Wakeup).expect("send wakeup");
    harness.run_steps(1);

    let commands = &harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .expect("root viewport output")
        .commands;
    assert_eq!(harness.state().state_label(), "Idle");
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Visible(true))),
        "expected wakeup to show the viewport, got {commands:?}"
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Focus)),
        "expected wakeup to focus the viewport, got {commands:?}"
    );
}

#[test]
fn exit_command_stops_task_runner() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    submit_text(&mut harness, "/exit");

    assert!(!harness.state().task_runner_state.is_running());
    assert_eq!(harness.state().state_label(), "Command");
    assert_eq!(
        harness.state().input_notice.as_deref(),
        Some("Stopping Donna.")
    );
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
fn chat_message_uses_selected_ai_provider() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
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
    let initial_count = harness.state().chat.messages().len();

    submit_text(&mut harness, "hello");
    harness.run_steps(8);

    let messages = harness.state().chat.messages();
    assert_eq!(messages.len(), initial_count + 3);
    assert!(
        messages
            .iter()
            .any(|message| message.text == "Mock response")
    );
    assert!(
        messages
            .last()
            .expect("name prompt")
            .text
            .contains("what should I call you")
    );
}

#[test]
fn asks_for_name_after_response_when_unknown() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
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

    submit_text(&mut harness, "hello");
    harness.run_steps(8);

    assert!(
        harness
            .state()
            .chat
            .messages()
            .last()
            .expect("name prompt")
            .text
            .contains("what should I call you")
    );
}

#[test]
fn does_not_ask_for_name_after_user_provides_it() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
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

    submit_text(&mut harness, "My name is Frieder");
    harness.run_steps(8);

    assert!(
        !harness
            .state()
            .chat
            .messages()
            .iter()
            .any(|message| message.text.contains("what should I call you"))
    );
}

#[test]
fn chat_prompt_includes_persisted_memories() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
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

    submit_text(&mut harness, "My name is Frieder");
    harness.run_steps(8);
    submit_text(&mut harness, "What is my name?");
    harness.run_steps(8);

    let memories = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .recent_memories(10)
        .expect("memories");

    assert!(
        memories
            .iter()
            .any(|memory| memory.content == "User name: Frieder")
    );

    let system_prompt = harness
        .state_mut()
        .system_prompt_with_memories("base".to_owned());
    assert!(system_prompt.contains("User name: Frieder"));
}

#[test]
fn todo_query_without_open_todos_uses_model_tool() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: "mock-response:{\"tool\":\"list_open_todos\",\"arguments\":{}}".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });
    let initial_count = harness.state().chat.messages().len();

    submit_text(&mut harness, "what are my todos?");
    harness.run_steps(8);

    let messages = harness.state().chat.messages();
    assert!(messages.len() >= initial_count + 2);
    assert!(
        messages
            .iter()
            .any(|message| message.text == "You have no open todos.")
    );
    assert!(!harness.state().response_in_progress);
}

#[test]
fn what_do_i_need_to_do_is_query_not_new_todo() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: "mock-response:{\"tool\":\"list_open_todos\",\"arguments\":{}}".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });

    submit_text(&mut harness, "what do I need to do?");
    harness.run_steps(8);

    let store = harness.state().store.as_ref().expect("store");
    assert!(store.open_todos(10).expect("todos").is_empty());
    assert!(
        harness
            .state()
            .chat
            .messages()
            .iter()
            .any(|message| message.text == "You have no open todos.")
    );
}

#[test]
fn todo_query_tool_lists_only_stored_open_todos() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: "mock-response:{\"tool\":\"list_open_todos\",\"arguments\":{}}".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });
    harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "file receipts".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "middle".to_owned(),
            due_at: None,
        })
        .expect("todo");

    submit_text(&mut harness, "show my todos");
    harness.run_steps(8);

    let answer = harness
        .state()
        .chat
        .messages()
        .iter()
        .find(|message| message.text.contains("file receipts"))
        .expect("tool answer")
        .text
        .as_str();
    assert!(answer.contains("file receipts"));
    assert!(!answer.contains("buy milk"));
}

#[test]
fn chat_can_change_open_todo_severity() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: "mock-response:{\"tool\":\"update_todo_severity\",\"arguments\":{\"todo_id\":1,\"severity\":\"high\"}}".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });
    let todo = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "file receipts".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "middle".to_owned(),
            due_at: None,
        })
        .expect("todo");

    submit_text(&mut harness, "make file receipts high priority");
    harness.run_steps(8);

    let store = harness.state().store.as_ref().expect("store");
    assert_eq!(store.todo(todo.id).expect("updated todo").severity, "high");
    assert!(
        harness
            .state()
            .chat
            .messages()
            .iter()
            .any(|message| { message.text == "Set 'file receipts' to high priority." })
    );
}

#[test]
fn chat_can_answer_open_todo_severity() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: "mock-response:{\"tool\":\"list_open_todos\",\"arguments\":{}}".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });
    harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "file receipts".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "high".to_owned(),
            due_at: None,
        })
        .expect("todo");

    submit_text(&mut harness, "what is the severity of file receipts?");
    harness.run_steps(8);

    assert!(harness.state().chat.messages().iter().any(|message| {
        message.text == "You have one open todo: file receipts. It is high priority."
    }));
}

#[test]
fn model_tool_lists_open_todos_with_severity() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: "mock-response:{\"tool\":\"list_open_todos\",\"arguments\":{}}".to_owned(),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });
    let store = harness.state().store.as_ref().expect("store");
    for (title, severity) in [("file receipts", "high"), ("send receipts", "low")] {
        store
            .create_todo(&NewTodo {
                title: title.to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: severity.to_owned(),
                due_at: None,
            })
            .expect("todo");
    }

    submit_text(&mut harness, "what severity had the todo you showed me");
    harness.run_steps(8);

    let answer = harness
        .state()
        .chat
        .messages()
        .iter()
        .find(|message| message.text.contains("file receipts"))
        .expect("tool answer")
        .text
        .as_str();
    assert!(answer.contains("file receipts (high priority)"));
    assert!(answer.contains("send receipts (low priority)"));
}

#[test]
fn embedded_tool_call_response_is_rendered_as_human_text() {
    let model_reply = "{\"tool\":\"list_open_todos\",\"arguments\":{}}\n\nid=1 severity=high title=raw prompt leak";
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        config.ai.chat.selected_model = "mock-tool".to_owned();
        config.ai.models.push(ModelConfig {
            id: "mock-tool".to_owned(),
            label: "Mock tool".to_owned(),
            provider: "mock".to_owned(),
            model: format!("mock-response:{model_reply}"),
            base_url: Some("mock://local".to_owned()),
            secret_ref: None,
        });
    });
    harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "fix the thing".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "high".to_owned(),
            due_at: None,
        })
        .expect("todo");

    submit_text(&mut harness, "show me my todos");
    harness.run_steps(8);

    let answer = harness
        .state()
        .chat
        .messages()
        .iter()
        .find(|message| message.text.contains("fix the thing"))
        .expect("tool answer")
        .text
        .as_str();
    assert_eq!(
        answer,
        "You have one open todo: fix the thing. It is high priority."
    );
    assert!(!answer.contains("tool"));
    assert!(!answer.contains("id="));
}

#[test]
fn startup_greeting_can_use_remembered_name_and_role() {
    let memories = vec![
        test_memory("User name: Frieder", 3),
        test_memory("Fact: User role: developer", 1),
        test_memory("Fact: User workplace: Acme", 1),
    ];
    let welcome = super::personalized_welcome(&memories).expect("welcome");

    assert!(welcome.contains("Frieder"));
    assert!(welcome.contains("developer"));
    assert!(welcome.contains("Acme"));
}

fn test_memory(content: &str, importance: i64) -> StoredMemory {
    StoredMemory {
        id: importance,
        memory_type: "fact".to_owned(),
        content: content.to_owned(),
        source: "donna_chat".to_owned(),
        confidence: 1.0,
        importance,
        created_at: 0,
        updated_at: 0,
        expires_at: None,
        forgotten_at: None,
    }
}

#[test]
fn chat_submission_enters_thinking_state_before_response_finishes() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
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

    submit_text(&mut harness, "hello");

    assert!(harness.state().response_in_progress);
    assert!(harness.state().state_label().starts_with("thinking"));
    assert_eq!(
        harness
            .state()
            .chat
            .messages()
            .last()
            .expect("placeholder")
            .text,
        "..."
    );
}

#[test]
fn thinking_chat_placeholder_uses_only_animated_dots() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        let model = config
            .ai
            .models
            .iter_mut()
            .find(|model| model.id == "ollama-local")
            .expect("ollama model");
        model.base_url = Some("http://10.255.255.1:11434".to_owned());
    });

    submit_text(&mut harness, "hello");
    harness.run_steps(1);

    let placeholder = &harness
        .state()
        .chat
        .messages()
        .last()
        .expect("placeholder")
        .text;
    assert!(matches!(placeholder.as_str(), "." | ".." | "..."));
}

#[test]
fn ollama_unavailable_shows_inline_chat_error() {
    let (_config_dir, mut harness) = app_harness_with_config(Vec2::new(720.0, 480.0), |config| {
        let model = config
            .ai
            .models
            .iter_mut()
            .find(|model| model.id == "ollama-local")
            .expect("ollama model");
        model.base_url = Some("http://127.0.0.1:9".to_owned());
    });

    submit_text(&mut harness, "hello");
    harness.run_steps(8);

    assert!(
        harness
            .state()
            .chat
            .messages()
            .iter()
            .any(|message| message.text.contains("Ollama local could not answer")),
    );
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
fn enter_executes_exact_typed_command() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    harness.state_mut().input = "/hide".to_owned();
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(1);

    assert_eq!(harness.state().input, "");
    assert_eq!(harness.state().state_label(), "Hidden");
}

#[test]
fn logic_handles_command_enter_before_text_input() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    harness.state_mut().input = "/hide".to_owned();
    harness.key_press(egui::Key::Enter);
    harness.step();

    assert_eq!(harness.state().input, "");
    assert_eq!(harness.state().state_label(), "Hidden");
}

#[test]
fn enter_submits_chat_input_without_losing_focus() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let initial_count = harness.state().chat.messages().len();

    harness.state_mut().input = "hello".to_owned();
    harness.run_steps(1);
    harness.key_press(egui::Key::Enter);
    harness.run_steps(1);

    assert!(harness.state().input.is_empty());
    assert!(harness.state().chat.messages().len() > initial_count);
    assert!(
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .accesskit_node()
            .is_focused(),
        "input should stay focused after sending"
    );
}

#[test]
fn tab_switches_model_without_moving_input_focus() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    harness.run_steps(1);
    assert_eq!(harness.state().selected_model_id, "ollama-local");

    harness.key_press(egui::Key::Tab);
    harness.run_steps(1);

    assert_eq!(harness.state().selected_model_id, "openai-compatible");
    assert!(
        harness
            .get_by_role(egui::accesskit::Role::TextInput)
            .accesskit_node()
            .is_focused(),
        "input should stay focused after model switch"
    );
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
fn builtin_todo_reminder_task_creates_attention_item() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let todo_id = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "urgent tax filing".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "high".to_owned(),
            due_at: None,
        })
        .expect("todo")
        .id;

    harness.state_mut().run_due_local_tasks_at(
        1_000,
        CronDateTime {
            minute: 10,
            hour: 8,
            day_of_month: 1,
            month: 1,
            day_of_week: 1,
        },
    );

    let items = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .ready_attention_items(1_000)
        .expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source_type, "todo_reminder");
    assert_eq!(items[0].source_id, Some(todo_id));
    assert_eq!(items[0].body.as_deref(), Some("urgent tax filing"));
}

#[test]
fn task_command_runs_todo_reminder_now() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let todo_id = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "send invoice".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "high".to_owned(),
            due_at: None,
        })
        .expect("todo")
        .id;

    submit_text(&mut harness, "/task todo_reminder");

    let items = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .ready_attention_items(i64::MAX)
        .expect("items");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].source_id, Some(todo_id));
    assert_eq!(
        harness.state().input_notice.as_deref(),
        Some("Task todo-reminder ran. Reminder #1 created.")
    );
}

#[test]
fn task_command_reports_usage_and_unknown_task() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));

    submit_text(&mut harness, "/task");
    assert_eq!(
        harness.state().input_error.as_deref(),
        Some("Usage: /task [name]")
    );

    submit_text(&mut harness, "/task missing");
    assert_eq!(
        harness.state().input_error.as_deref(),
        Some("Unknown task: missing")
    );
}

#[test]
fn forget_command_clears_task_reminder_snoozes() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let todo = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "ignored invoice".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "middle".to_owned(),
            due_at: None,
        })
        .expect("todo");
    let item = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_attention_item(&NewAttentionItem {
            source_type: "todo_reminder".to_owned(),
            source_id: Some(todo.id),
            level: "normal".to_owned(),
            title: "Open todo".to_owned(),
            body: Some("ignored invoice".to_owned()),
            due_at: None,
            payload: None,
        })
        .expect("attention");
    harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .snooze_todo_until(todo.id, 2_000)
        .expect("snooze todo");
    harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .snooze_attention_item(item.id, 2_000)
        .expect("snooze attention");

    submit_text(&mut harness, "/forget");

    assert_eq!(
        harness.state().input_notice.as_deref(),
        Some("Forgot 2 task snooze records.")
    );
    assert_eq!(
        harness
            .state()
            .store
            .as_ref()
            .expect("store")
            .todo(todo.id)
            .expect("todo")
            .snoozed_until,
        None
    );
}

#[test]
fn attention_restores_hidden_ui_without_exiting() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    submit_text(&mut harness, "/hide");
    harness.run_steps(1);

    harness
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
        .expect("attention");

    harness.run_steps(1);

    let commands = &harness
        .output()
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .expect("root viewport output")
        .commands;
    assert!(harness.state().task_runner_state.is_running());
    assert_eq!(harness.state().state_label(), "Attention");
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Visible(true))),
        "expected attention to show the viewport, got {commands:?}"
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Minimized(false))),
        "expected attention to unminimize the viewport, got {commands:?}"
    );
}

#[test]
fn todo_reminder_attention_card_uses_requested_actions() {
    let (_config_dir, mut harness) = app_harness(Vec2::new(720.0, 480.0));
    let todo = harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_todo(&NewTodo {
            title: "reply to Tim".to_owned(),
            notes: None,
            source: "test".to_owned(),
            related_topic: None,
            severity: "middle".to_owned(),
            due_at: None,
        })
        .expect("todo");
    harness
        .state()
        .store
        .as_ref()
        .expect("store")
        .create_attention_item(&NewAttentionItem {
            source_type: "todo_reminder".to_owned(),
            source_id: Some(todo.id),
            level: "normal".to_owned(),
            title: "Open todo".to_owned(),
            body: Some(todo.title),
            due_at: None,
            payload: None,
        })
        .expect("attention");

    harness.run_steps(1);

    assert!(harness.query_by_label("Already done").is_some());
    assert!(harness.query_by_label("Later today").is_some());
    assert!(harness.query_by_label("Tomorrow").is_some());
    assert!(harness.query_by_label("Ignore").is_some());
    assert!(harness.query_by_label("Done").is_none());
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
