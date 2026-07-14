mod calendar_create_appointment;
mod calendar_delete_appointment;
mod calendar_list_appointments;
mod calendar_move_appointment;
mod calendar_search_appointments;
mod complete_todo;
mod create_todo;
mod delete_todo;
mod list_tomorrow_appointments;
mod list_completed_todos;
mod list_open_todos;
mod outlook_list_mails;
mod outlook_search_mails;
mod outlook_send_mail;
mod shared;
mod summarize_teams_conversation;
mod teams_list_channel_messages;
mod teams_list_channels;
mod teams_list_chat_messages;
mod teams_list_chats;
mod teams_search_messages;
mod teams_send_message;
mod update_todo_due_at;
mod update_todo_severity;

use donna_storage::LocalStore;
use serde::Deserialize;

const LOCAL_TOOLS_PREAMBLE: &str = include_str!("../../../../assets/tools/preamble.md");
const LIST_OPEN_TODOS_PROMPT: &str = include_str!("../../../../assets/tools/list_open_todos.md");
const LIST_COMPLETED_TODOS_PROMPT: &str =
    include_str!("../../../../assets/tools/list_completed_todos.md");
const CREATE_TODO_PROMPT: &str = include_str!("../../../../assets/tools/create_todo.md");
const COMPLETE_TODO_PROMPT: &str = include_str!("../../../../assets/tools/complete_todo.md");
const DELETE_TODO_PROMPT: &str = include_str!("../../../../assets/tools/delete_todo.md");
const UPDATE_TODO_SEVERITY_PROMPT: &str =
    include_str!("../../../../assets/tools/update_todo_severity.md");
const UPDATE_TODO_DUE_AT_PROMPT: &str =
    include_str!("../../../../assets/tools/update_todo_due_at.md");
const SUMMARIZE_TEAMS_CONVERSATION_PROMPT: &str =
    include_str!("../../../../assets/tools/summarize_teams_conversation.md");
const LIST_TOMORROW_APPOINTMENTS_PROMPT: &str =
    include_str!("../../../../assets/tools/list_tomorrow_appointments.md");
const OUTLOOK_LIST_MAILS_PROMPT: &str =
    include_str!("../../../../assets/tools/outlook_list_mails.md");
const OUTLOOK_SEARCH_MAILS_PROMPT: &str =
    include_str!("../../../../assets/tools/outlook_search_mails.md");
const OUTLOOK_SEND_MAIL_PROMPT: &str =
    include_str!("../../../../assets/tools/outlook_send_mail.md");
const TEAMS_LIST_CHAT_MESSAGES_PROMPT: &str =
    include_str!("../../../../assets/tools/teams_list_chat_messages.md");
const TEAMS_LIST_CHANNEL_MESSAGES_PROMPT: &str =
    include_str!("../../../../assets/tools/teams_list_channel_messages.md");
const TEAMS_LIST_CHATS_PROMPT: &str =
    include_str!("../../../../assets/tools/teams_list_chats.md");
const TEAMS_LIST_CHANNELS_PROMPT: &str =
    include_str!("../../../../assets/tools/teams_list_channels.md");
const TEAMS_SEARCH_MESSAGES_PROMPT: &str =
    include_str!("../../../../assets/tools/teams_search_messages.md");
const TEAMS_SEND_MESSAGE_PROMPT: &str =
    include_str!("../../../../assets/tools/teams_send_message.md");
const CALENDAR_LIST_APPOINTMENTS_PROMPT: &str =
    include_str!("../../../../assets/tools/calendar_list_appointments.md");
const CALENDAR_SEARCH_APPOINTMENTS_PROMPT: &str =
    include_str!("../../../../assets/tools/calendar_search_appointments.md");
const CALENDAR_CREATE_APPOINTMENT_PROMPT: &str =
    include_str!("../../../../assets/tools/calendar_create_appointment.md");
const CALENDAR_DELETE_APPOINTMENT_PROMPT: &str =
    include_str!("../../../../assets/tools/calendar_delete_appointment.md");
const CALENDAR_MOVE_APPOINTMENT_PROMPT: &str =
    include_str!("../../../../assets/tools/calendar_move_appointment.md");
const LOCAL_TOOLS_GUARDRAILS: &str = include_str!("../../../../assets/tools/guardrails.md");

#[derive(Debug, Deserialize)]
pub(super) struct ModelToolCall {
    #[serde(alias = "name", alias = "function")]
    tool: String,
    #[serde(default, alias = "args", alias = "input", alias = "parameters")]
    pub(super) arguments: serde_json::Value,
    #[serde(flatten)]
    pub(super) extra: serde_json::Map<String, serde_json::Value>,
}

pub fn local_tool_prompt() -> String {
    let mut prompt = String::from(LOCAL_TOOLS_PREAMBLE);
    for tool_prompt in [
        LIST_OPEN_TODOS_PROMPT,
        LIST_COMPLETED_TODOS_PROMPT,
        CREATE_TODO_PROMPT,
        COMPLETE_TODO_PROMPT,
        DELETE_TODO_PROMPT,
        UPDATE_TODO_SEVERITY_PROMPT,
        UPDATE_TODO_DUE_AT_PROMPT,
        SUMMARIZE_TEAMS_CONVERSATION_PROMPT,
        LIST_TOMORROW_APPOINTMENTS_PROMPT,
        OUTLOOK_LIST_MAILS_PROMPT,
        OUTLOOK_SEARCH_MAILS_PROMPT,
        OUTLOOK_SEND_MAIL_PROMPT,
        TEAMS_LIST_CHAT_MESSAGES_PROMPT,
        TEAMS_LIST_CHANNEL_MESSAGES_PROMPT,
        TEAMS_LIST_CHATS_PROMPT,
        TEAMS_LIST_CHANNELS_PROMPT,
        TEAMS_SEARCH_MESSAGES_PROMPT,
        TEAMS_SEND_MESSAGE_PROMPT,
        CALENDAR_LIST_APPOINTMENTS_PROMPT,
        CALENDAR_SEARCH_APPOINTMENTS_PROMPT,
        CALENDAR_CREATE_APPOINTMENT_PROMPT,
        CALENDAR_DELETE_APPOINTMENT_PROMPT,
        CALENDAR_MOVE_APPOINTMENT_PROMPT,
    ] {
        prompt.push_str(tool_prompt.trim());
        prompt.push('\n');
    }
    prompt.push_str(LOCAL_TOOLS_GUARDRAILS);
    prompt
}

pub fn execute_tool_call_from_model(
    store: Option<&LocalStore>,
    text: &str,
    user_message: &str,
) -> Option<String> {
    let Some(store) = store else {
        return Some("I cannot see a local todo store right now.".to_owned());
    };

    if let Some(calls) = parse_model_tool_calls(text) {
        let results = calls
            .into_iter()
            .map(|call| execute_model_tool_call(store, call, user_message))
            .collect::<Vec<_>>();

        return Some(results.join("\n"));
    }

    if should_force_latest_outlook_mail(user_message, text) {
        let call = ModelToolCall {
            tool: "outlook_list_mails".to_owned(),
            arguments: serde_json::json!({ "limit": 1 }),
            extra: serde_json::Map::new(),
        };
        return Some(execute_model_tool_call(store, call, user_message));
    }

    None
}

pub fn humanize_model_todo_leak(store: Option<&LocalStore>, text: String) -> String {
    if !looks_like_raw_todo_context(&text) {
        return text;
    }
    let Some(store) = store else {
        return "I cannot see a local todo store right now.".to_owned();
    };

    match store.open_todos(20) {
        Ok(todos) => shared::format_open_todo_list(&todos),
        Err(error) => format!("I cannot read your todos: {error}"),
    }
}

fn execute_model_tool_call(store: &LocalStore, call: ModelToolCall, user_message: &str) -> String {
    let tool = normalize_tool_name(&call.tool);
    match tool.as_str() {
        "list_open_todos" => list_open_todos::execute(store),
        "list_completed_todos" => list_completed_todos::execute(store),
        "create_todo" => create_todo::execute(store, &call, user_message),
        "complete_todo" => complete_todo::execute(store, &call),
        "delete_todo" => delete_todo::execute(store, &call),
        "update_todo_severity" => update_todo_severity::execute(store, &call),
        "update_todo_due_at" => update_todo_due_at::execute(store, &call),
        "summarize_teams_conversation" => summarize_teams_conversation::execute(store, &call),
        "list_tomorrow_appointments" => list_tomorrow_appointments::execute(store),
        "outlook_list_mails" => outlook_list_mails::execute(store, &call, user_message),
        "outlook_search_mails" => outlook_search_mails::execute(store, &call),
        "outlook_send_mail" => outlook_send_mail::execute(&call),
        "teams_list_chat_messages" => teams_list_chat_messages::execute(store, &call),
        "teams_list_channel_messages" => teams_list_channel_messages::execute(store, &call),
        "teams_list_chats" => teams_list_chats::execute(store, &call),
        "teams_list_channels" => teams_list_channels::execute(store, &call),
        "teams_search_messages" => teams_search_messages::execute(store, &call),
        "teams_send_message" => teams_send_message::execute(&call),
        "calendar_list_appointments" => calendar_list_appointments::execute(store, &call),
        "calendar_search_appointments" => calendar_search_appointments::execute(store, &call),
        "calendar_create_appointment" => calendar_create_appointment::execute(&call),
        "calendar_delete_appointment" => calendar_delete_appointment::execute(&call),
        "calendar_move_appointment" => calendar_move_appointment::execute(&call),
        _ => "I do not know that local tool.".to_owned(),
    }
}

fn normalize_tool_name(tool: &str) -> String {
    match tool.trim().to_ascii_lowercase().as_str() {
        "add_todo" | "new_todo" => "create_todo".to_owned(),
        "done_todo" | "mark_todo_done" | "mark_done" | "complete_task" => {
            "complete_todo".to_owned()
        }
        "remove_todo" | "dismiss_todo" | "delete_task" | "remove_task" => "delete_todo".to_owned(),
        "set_todo_severity" | "set_todo_priority" | "update_todo_priority" => {
            "update_todo_severity".to_owned()
        }
        "set_todo_due_at" | "set_todo_due" | "update_todo_due" | "update_todo_due_date" => {
            "update_todo_due_at".to_owned()
        }
        "completed_todo"
        | "completed_todos"
        | "done_todos"
        | "list_completed_todo"
        | "list_done_todo"
        | "list_done_todos"
        | "any_completed_todo"
        | "any_completed_todos" => "list_completed_todos".to_owned(),
        "list_todos" | "show_todos" => "list_open_todos".to_owned(),
        "teams_summary" | "summarize_teams" | "summarize_chat" => {
            "summarize_teams_conversation".to_owned()
        }
        "tomorrow_appointments" | "list_tomorrow_events" | "list_tomorrow_calendar" => {
            "list_tomorrow_appointments".to_owned()
        }
        other => other.to_owned(),
    }
}

fn parse_model_tool_calls(text: &str) -> Option<Vec<ModelToolCall>> {
    let trimmed = text.trim();
    if let Some(call) = parse_trace_style_tool_call(trimmed) {
        return Some(vec![call]);
    }
    let json = trimmed
        .strip_prefix("```json")
        .and_then(|text| text.strip_suffix("```"))
        .or_else(|| {
            trimmed
                .strip_prefix("```")
                .and_then(|text| text.strip_suffix("```"))
        })
        .unwrap_or(trimmed)
        .trim();

    parse_tool_calls_json(json)
        .ok()
        .or_else(|| parse_embedded_model_tool_calls(json))
}

fn parse_tool_calls_json(json: &str) -> Result<Vec<ModelToolCall>, serde_json::Error> {
    serde_json::from_str::<Vec<ModelToolCall>>(json)
        .or_else(|_| serde_json::from_str::<ModelToolCall>(json).map(|call| vec![call]))
}

fn parse_embedded_model_tool_calls(text: &str) -> Option<Vec<ModelToolCall>> {
    let bytes = text.as_bytes();
    let mut start = None;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, byte) in bytes.iter().enumerate() {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'\"' {
                in_string = false;
            }
            continue;
        }

        match *byte {
            b'\"' => in_string = true,
            b'{' => {
                if depth == 0 {
                    start = Some(index);
                }
                depth += 1;
            }
            b'}' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let object_start = start?;
                    let candidate = &text[object_start..=index];
                    if let Ok(calls) = parse_tool_calls_json(candidate) {
                        return Some(calls);
                    }
                    start = None;
                }
            }
            _ => {}
        }
    }

    None
}

fn parse_trace_style_tool_call(text: &str) -> Option<ModelToolCall> {
    let line = text.lines().next()?.trim();
    let prefix = "Using tool:";
    let remainder = line.strip_prefix(prefix)?.trim();
    let (tool, arguments) = remainder.split_once(", arguments:")?;
    let arguments = arguments.trim();
    let arguments = serde_json::from_str::<serde_json::Value>(arguments)
        .ok()
        .or_else(|| parse_relaxed_object_literal(arguments))?;

    Some(ModelToolCall {
        tool: tool.trim().to_owned(),
        arguments,
        extra: serde_json::Map::new(),
    })
}

fn parse_relaxed_object_literal(text: &str) -> Option<serde_json::Value> {
    let trimmed = text.trim();
    let inner = trimmed.strip_prefix('{')?.strip_suffix('}')?.trim();
    if inner.is_empty() {
        return Some(serde_json::Value::Object(serde_json::Map::new()));
    }

    let mut object = serde_json::Map::new();
    for pair in inner.split(',') {
        let (key, raw_value) = pair.split_once(':')?;
        let key = key.trim().trim_matches('"').trim_matches('\'');
        let raw_value = raw_value.trim();
        let value = if raw_value.starts_with('"') && raw_value.ends_with('"') {
            serde_json::Value::String(raw_value.trim_matches('"').to_owned())
        } else if raw_value.starts_with('\'') && raw_value.ends_with('\'') {
            serde_json::Value::String(raw_value.trim_matches('\'').to_owned())
        } else if raw_value.eq_ignore_ascii_case("null") {
            serde_json::Value::Null
        } else if raw_value.eq_ignore_ascii_case("true") {
            serde_json::Value::Bool(true)
        } else if raw_value.eq_ignore_ascii_case("false") {
            serde_json::Value::Bool(false)
        } else if let Ok(number) = raw_value.parse::<i64>() {
            serde_json::Value::Number(number.into())
        } else if let Ok(number) = raw_value.parse::<u64>() {
            serde_json::Value::Number(number.into())
        } else if let Ok(number) = raw_value.parse::<f64>() {
            serde_json::json!(number)
        } else {
            serde_json::Value::String(raw_value.to_owned())
        };
        object.insert(key.to_owned(), value);
    }

    Some(serde_json::Value::Object(object))
}

fn looks_like_raw_todo_context(text: &str) -> bool {
    let mut raw_lines = 0;
    for line in text.lines() {
        let line = line.trim();
        let line = line.strip_prefix("- ").unwrap_or(line).trim_start();
        if line.starts_with("id=") && line.contains("title=") {
            raw_lines += 1;
        }
    }

    raw_lines > 0
}

fn should_force_latest_outlook_mail(user_message: &str, model_text: &str) -> bool {
    let question = user_message.to_ascii_lowercase();
    let mentions_latest_mail = (question.contains("last mail")
        || question.contains("latest mail")
        || question.contains("recent mail")
        || question.contains("last email")
        || question.contains("latest email")
        || question.contains("recent email"))
        && !question.contains("before")
        && !question.contains("after")
        && !question.contains("between")
        && !question.contains("today")
        && !question.contains("yesterday")
        && !question.contains("this week")
        && !question.contains("last week")
        && !question.contains("this month")
        && !question.contains("last month");

    let fabricated_mail_shape = model_text.contains("\"subject\"")
        && (model_text.contains("\"timeReceived\"")
            || model_text.contains("\"timeSent\"")
            || model_text.contains("\"from\""));

    mentions_latest_mail && fabricated_mail_shape
}

#[cfg(test)]
mod tests {
    use super::{parse_model_tool_calls, parse_trace_style_tool_call};

    #[test]
    fn parses_trace_style_tool_call_prefix() {
        let text = "Using tool: outlook_list_mails, arguments: {\"date_from\":1712448000,\"date_to\":1715126400,\"limit\":1}\nResult: {\"subject\":\"fake\"}";
        let call = parse_trace_style_tool_call(text).expect("call");

        assert_eq!(call.tool, "outlook_list_mails");
        assert_eq!(call.arguments["limit"], 1);
    }

    #[test]
    fn parses_trace_style_tool_call_with_unquoted_keys() {
        let text = "Using tool: outlook_list_mails, arguments: {date_from: 1712448000, date_to: 1715126400, limit: 1}\nResult: {\"subject\":\"fake\"}";
        let call = parse_trace_style_tool_call(text).expect("call");

        assert_eq!(call.tool, "outlook_list_mails");
        assert_eq!(call.arguments["limit"], 1);
        assert_eq!(call.arguments["date_from"], 1712448000);
    }

    #[test]
    fn parse_model_tool_calls_prefers_trace_style_prefix() {
        let text = "Using tool: outlook_list_mails, arguments: {\"limit\":1}\nResult: {\"subject\":\"fake\"}";
        let calls = parse_model_tool_calls(text).expect("calls");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "outlook_list_mails");
        assert_eq!(calls[0].arguments["limit"], 1);
    }

    #[test]
    fn forces_latest_outlook_mail_on_fabricated_mail_json() {
        assert!(super::should_force_latest_outlook_mail(
            "what was the last mail I received?",
            "{\"subject\":\"Status update\",\"from\":{\"name\":\"Sender\"},\"timeReceived\":1715143200}"
        ));
        assert!(!super::should_force_latest_outlook_mail(
            "what mails did I get last week?",
            "{\"subject\":\"Status update\",\"from\":{\"name\":\"Sender\"},\"timeReceived\":1715143200}"
        ));
    }
}
