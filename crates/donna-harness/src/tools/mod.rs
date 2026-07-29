mod calendar_create_appointment;
mod calendar_delete_appointment;
mod calendar_list_appointments;
mod calendar_move_appointment;
mod calendar_search_appointments;
mod complete_todo;
mod create_todo;
mod delete_todo;
mod list_completed_todos;
mod list_open_todos;
mod list_today_appointments;
mod list_tomorrow_appointments;
mod next_appointment;
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
const LIST_TODAY_APPOINTMENTS_PROMPT: &str =
    include_str!("../../../../assets/tools/list_today_appointments.md");
const LIST_TOMORROW_APPOINTMENTS_PROMPT: &str =
    include_str!("../../../../assets/tools/list_tomorrow_appointments.md");
const NEXT_APPOINTMENT_PROMPT: &str =
    include_str!("../../../../assets/tools/next_appointment.md");
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
    #[serde(alias = "name", alias = "function", alias = "type")]
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
        LIST_TODAY_APPOINTMENTS_PROMPT,
        LIST_TOMORROW_APPOINTMENTS_PROMPT,
        NEXT_APPOINTMENT_PROMPT,
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

/// Appends the event_id of the single specific appointment `model_text`
/// answered about, when it can be determined, to the text that gets stored
/// as Donna's reply and fed back to the model as conversation history. This
/// makes the reference survive across turns deterministically, instead of
/// depending on a local model to notice, remember, and retype a structured
/// id it was explicitly told not to speak aloud — see
/// `format_appointment_line` for where the tag originates and
/// `execute_model_tool_call`'s event_id fast path for where it gets consumed
/// on the next turn. A single "[event_id: N]" tag in `raw_tool_result`
/// settles it outright; several tags (the tool result covered multiple
/// candidate appointments) are disambiguated by matching the "(start - end)"
/// time window the model already copied verbatim into its own answer, per
/// the follow-up prompt's instruction to answer only from tool-result facts.
/// Returns `model_text` unchanged when neither resolves to exactly one id.
pub fn tag_calendar_followup_with_event_id(model_text: String, raw_tool_result: &str) -> String {
    let event_id = shared::referenced_event_id(raw_tool_result)
        .or_else(|| shared::matching_event_id_by_time(&model_text, raw_tool_result));
    match event_id {
        Some(event_id) => format!("{model_text}\n[event_id: {event_id}]"),
        None => model_text,
    }
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

fn execute_model_tool_call(store: &LocalStore, mut call: ModelToolCall, user_message: &str) -> String {
    let normalized = normalize_tool_name(&call.tool);
    let has_event_id =
        shared::event_id_argument(&shared::normalized_call_arguments(&call)).is_some();

    // A call carrying event_id ("who's attending that meeting?") is asking
    // about one exact event the model already knows the id of — keep it on
    // calendar_search_appointments (which has the event_id fast path)
    // rather than letting the day-keyword redirect below throw that precise
    // reference away.
    let tool = if has_event_id && is_calendar_tool(&normalized) {
        "calendar_search_appointments".to_owned()
    } else {
        correct_appointment_tool_for_user_message(normalized, user_message)
    };

    if tool == "calendar_search_appointments" && !has_event_id {
        correct_calendar_search_date_range(&mut call, user_message);
    }
    let result = match tool.as_str() {
        "list_open_todos" => list_open_todos::execute(store),
        "list_completed_todos" => list_completed_todos::execute(store),
        "create_todo" => create_todo::execute(store, &call, user_message),
        "complete_todo" => complete_todo::execute(store, &call),
        "delete_todo" => delete_todo::execute(store, &call),
        "update_todo_severity" => update_todo_severity::execute(store, &call),
        "update_todo_due_at" => update_todo_due_at::execute(store, &call),
        "summarize_teams_conversation" => summarize_teams_conversation::execute(store, &call),
        "list_today_appointments" => list_today_appointments::execute(store, user_message),
        "list_tomorrow_appointments" => list_tomorrow_appointments::execute(store, user_message),
        "next_appointment" => next_appointment::execute(store, &call, user_message),
        "outlook_list_mails" => outlook_list_mails::execute(store, &call, user_message),
        "outlook_search_mails" => outlook_search_mails::execute(store, &call),
        "outlook_send_mail" => outlook_send_mail::execute(&call),
        "teams_list_chat_messages" => teams_list_chat_messages::execute(store, &call),
        "teams_list_channel_messages" => teams_list_channel_messages::execute(store, &call),
        "teams_list_chats" => teams_list_chats::execute(store, &call),
        "teams_list_channels" => teams_list_channels::execute(store, &call),
        "teams_search_messages" => teams_search_messages::execute(store, &call),
        "teams_send_message" => teams_send_message::execute(&call),
        "calendar_list_appointments" => {
            calendar_list_appointments::execute(store, &call, user_message)
        }
        "calendar_search_appointments" => {
            calendar_search_appointments::execute(store, &call, user_message)
        }
        "calendar_create_appointment" => calendar_create_appointment::execute(&call),
        "calendar_delete_appointment" => calendar_delete_appointment::execute(&call),
        "calendar_move_appointment" => calendar_move_appointment::execute(&call),
        _ => "I do not know that local tool.".to_owned(),
    };
    eprintln!("{}", tool_call_log_line(&tool, &call.arguments, &result));
    result
}

/// Every model-invoked tool call is logged with its name, arguments, and
/// result so tool usage can be audited from the process log, per AGENTS.md.
fn tool_call_log_line(tool: &str, arguments: &serde_json::Value, result: &str) -> String {
    format!(
        "donna harness: tool call tool={tool} arguments={arguments} result={}",
        result.replace('\n', "\\n")
    )
}

fn is_calendar_tool(tool: &str) -> bool {
    matches!(
        tool,
        "calendar_search_appointments"
            | "calendar_list_appointments"
            | "next_appointment"
            | "list_today_appointments"
            | "list_tomorrow_appointments"
    )
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
        "today_appointments" | "list_today_events" | "list_today_calendar" => {
            "list_today_appointments".to_owned()
        }
        "next_meeting" | "next_event" | "upcoming_appointment" | "next_calendar_event" => {
            "next_appointment".to_owned()
        }
        other => other.to_owned(),
    }
}

/// Local models frequently mix up next_appointment/list_today_appointments/
/// list_tomorrow_appointments when the user names a specific day (e.g. "first
/// meeting tomorrow" gets routed to next_appointment), and sometimes reach
/// for the generic calendar_list_appointments tool with a hallucinated or
/// degenerate date range instead (e.g. date_from == date_to at the exact time
/// asked about, which then matches nothing since a real meeting's start/end
/// rarely lands on that exact second) — that tool has no way to know "today"
/// without the model doing correct date arithmetic, which local models are
/// unreliable at. calendar_list_appointments carries no other useful
/// arguments (just a date range and a limit), so redirecting it to the
/// day-specific tool loses nothing. Since the day the user asked about is
/// unambiguous text in their own message, override the model's tool choice
/// deterministically rather than relying on prompt wording alone.
///
/// calendar_search_appointments is handled separately by
/// `correct_calendar_search_date_range` instead of being redirected here: it
/// can carry a real `text`/`people` filter the model correctly extracted
/// (e.g. a person's name), and swapping it out for the unfiltered day list would
/// throw that filter away, leaving the model to eyeball a long list for a
/// name instead of letting the storage layer match it directly.
fn correct_appointment_tool_for_user_message(tool: String, user_message: &str) -> String {
    if !matches!(
        tool.as_str(),
        "next_appointment" | "list_today_appointments" | "list_tomorrow_appointments"
            | "calendar_list_appointments"
    ) {
        return tool;
    }

    let question = user_message.to_ascii_lowercase();
    let mentions_tomorrow = question.contains("tomorrow");
    let mentions_today = question.contains("today");

    match (mentions_tomorrow, mentions_today) {
        (true, false) => "list_tomorrow_appointments".to_owned(),
        (false, true) => "list_today_appointments".to_owned(),
        _ => tool,
    }
}

/// Overrides a calendar_search_appointments call's date_from/date_to with
/// the real "today"/"tomorrow" boundary when the user's message names one,
/// while leaving any text/people filter the model supplied untouched — see
/// `correct_appointment_tool_for_user_message` for why this tool is fixed up
/// in place instead of being redirected to a different tool.
fn correct_calendar_search_date_range(call: &mut ModelToolCall, user_message: &str) {
    let question = user_message.to_ascii_lowercase();
    let offset = if question.contains("tomorrow") {
        1
    } else if question.contains("today") {
        0
    } else {
        return;
    };

    let (day_start, day_end) = shared::day_bounds_for_offset(offset);
    let arguments = match call.arguments.as_object_mut() {
        Some(arguments) => arguments,
        None => {
            call.arguments = serde_json::Value::Object(serde_json::Map::new());
            call.arguments.as_object_mut().expect("just set to object")
        }
    };
    arguments.insert("date_from".to_owned(), serde_json::json!(day_start));
    arguments.insert("date_to".to_owned(), serde_json::json!(day_end));
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
                    // A local model sometimes wraps a well-formed tool call
                    // in a stray extra brace pair (e.g. "{{...}} }"), which
                    // makes the outermost balanced span invalid JSON (an
                    // object can't directly contain another object with no
                    // key) even though the object nested one level in is
                    // perfectly valid. Retry on the interior with the outer
                    // brace pair stripped before giving up on this span.
                    if candidate.len() > 2 {
                        if let Some(calls) =
                            parse_embedded_model_tool_calls(&candidate[1..candidate.len() - 1])
                        {
                            return Some(calls);
                        }
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
    use super::{
        ModelToolCall, execute_model_tool_call, parse_model_tool_calls, parse_trace_style_tool_call,
        tag_calendar_followup_with_event_id, tool_call_log_line,
    };
    use donna_storage::{LocalStore, NewCalendarEvent};

    #[test]
    fn list_today_appointments_finds_event_starting_within_today_but_not_tomorrow() {
        let store = LocalStore::in_memory().expect("store");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_secs() as i64;
        let day_start = (now.div_euclid(86_400)) * 86_400;

        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "today-event".to_owned(),
                subject: Some("Standup".to_owned()),
                organizer_name: None,
                organizer_email: None,
                starts_at: Some(day_start + 3_600),
                ends_at: Some(day_start + 7_200),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");
        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "tomorrow-event".to_owned(),
                subject: Some("Not today".to_owned()),
                organizer_name: None,
                organizer_email: None,
                starts_at: Some(day_start + 86_400 + 3_600),
                ends_at: Some(day_start + 86_400 + 7_200),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");

        let call = ModelToolCall {
            tool: "list_today_appointments".to_owned(),
            arguments: serde_json::json!({}),
            extra: serde_json::Map::new(),
        };
        let result = execute_model_tool_call(&store, call, "what's on today?");

        assert!(result.contains("Standup"));
        assert!(!result.contains("Not today"));
    }

    #[test]
    fn next_appointment_finds_soonest_upcoming_event_ignoring_the_past() {
        let store = LocalStore::in_memory().expect("store");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_secs() as i64;

        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "past-event".to_owned(),
                subject: Some("Already happened".to_owned()),
                organizer_name: None,
                organizer_email: None,
                starts_at: Some(now - 3_600),
                ends_at: Some(now - 1_800),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");
        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "soonest-event".to_owned(),
                subject: Some("Standup".to_owned()),
                organizer_name: None,
                organizer_email: None,
                starts_at: Some(now + 1_800),
                ends_at: Some(now + 3_600),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");
        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "later-event".to_owned(),
                subject: Some("Later".to_owned()),
                organizer_name: None,
                organizer_email: None,
                starts_at: Some(now + 7_200),
                ends_at: Some(now + 10_800),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");

        let call = ModelToolCall {
            tool: "next_appointment".to_owned(),
            arguments: serde_json::json!({}),
            extra: serde_json::Map::new(),
        };
        let result = execute_model_tool_call(&store, call, "what's my next meeting?");

        assert!(result.contains("Standup"));
        assert!(!result.contains("Already happened"));
        assert!(!result.contains("Later"));
    }

    #[test]
    fn next_appointment_with_persons_finds_soonest_matching_organizer_not_the_soonest_overall() {
        let store = LocalStore::in_memory().expect("store");
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_secs() as i64;

        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "daily-meeting".to_owned(),
                subject: Some("Daily meeting".to_owned()),
                organizer_name: None,
                organizer_email: None,
                starts_at: Some(now + 1_800),
                ends_at: Some(now + 3_600),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");
        store
            .upsert_calendar_event(&NewCalendarEvent {
                external_id: "alex-sync".to_owned(),
                subject: Some("Sync with Alex".to_owned()),
                organizer_name: Some("Alex Example".to_owned()),
                organizer_email: None,
                starts_at: Some(now + 7_200),
                ends_at: Some(now + 10_800),
                original_timezone: None,
                show_as: None,
                etag: None,
                change_key: None,
                is_cancelled: false,
                is_deleted: false,
                is_all_day: false,
                attendees: Vec::new(),
            })
            .expect("event");

        let call = ModelToolCall {
            tool: "next_appointment".to_owned(),
            arguments: serde_json::json!({ "persons": "alex" }),
            extra: serde_json::Map::new(),
        };
        let result = execute_model_tool_call(
            &store,
            call,
            "when is my next meeting with alex.example@example.com",
        );

        assert!(result.contains("Sync with Alex"));
        assert!(!result.contains("Daily meeting"));
    }

    #[test]
    fn tool_call_log_line_includes_tool_arguments_and_result_on_one_line() {
        let line = tool_call_log_line(
            "create_todo",
            &serde_json::json!({"title": "file receipts"}),
            "Created todo #3: file receipts\nDue: none",
        );

        assert!(line.contains("tool=create_todo"));
        assert!(line.contains("arguments={\"title\":\"file receipts\"}"));
        assert!(line.contains("result=Created todo #3: file receipts\\nDue: none"));
        assert!(!line.contains('\n'));
    }

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
    fn parse_model_tool_calls_recovers_from_stray_outer_brace() {
        let text = "I'll check that for you. {{\"tool\":\"calendar_search_appointments\",\"arguments\":{\"text\":\"alex today\",\"persons\":\"\",\"date_from\":1785253167,\"date_to\":1785253167,\"limit\":25}} }";
        let calls = parse_model_tool_calls(text).expect("calls");

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool, "calendar_search_appointments");
        assert_eq!(calls[0].arguments["limit"], 25);
    }

    #[test]
    fn tags_followup_with_the_single_referenced_event_id() {
        let raw_tool_result = "Found 1 matching synced appointments:\n- Standup (09:00 - 09:30) [event_id: 42]";
        let tagged =
            tag_calendar_followup_with_event_id("You have Standup at 9am.".to_owned(), raw_tool_result);

        assert_eq!(tagged, "You have Standup at 9am.\n[event_id: 42]");
    }

    #[test]
    fn tags_followup_by_matching_the_quoted_time_among_several_candidates() {
        // Mirrors the real repro: a day-wide search returns several
        // candidates, but the follow-up model's own prose settles on one by
        // quoting its time window — that should resolve unambiguously even
        // though `referenced_event_id` alone can't (three tags present).
        let raw_tool_result = "Found 3 matching synced appointments:\n\
             - VDS Feature Ready (1. Termin im Sprint) (10:00 - 11:30) [event_id: 1447]\n\
             - Deployment (15:00 - 16:55) [event_id: 2736]\n\
             - CCA Weekly 1 (17:00 - 18:00) [event_id: 1472]";
        let tagged = tag_calendar_followup_with_event_id(
            "The meeting with Alex is scheduled for today at 10:00 - 11:30.".to_owned(),
            raw_tool_result,
        );

        assert_eq!(
            tagged,
            "The meeting with Alex is scheduled for today at 10:00 - 11:30.\n[event_id: 1447]"
        );
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
