mod complete_todo;
mod create_todo;
mod delete_todo;
mod list_completed_todos;
mod list_open_todos;
mod shared;
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
    let calls = parse_model_tool_calls(text)?;
    let Some(store) = store else {
        return Some("I cannot see a local todo store right now.".to_owned());
    };

    let results = calls
        .into_iter()
        .map(|call| execute_model_tool_call(store, call, user_message))
        .collect::<Vec<_>>();

    Some(results.join("\n"))
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
        other => other.to_owned(),
    }
}

fn parse_model_tool_calls(text: &str) -> Option<Vec<ModelToolCall>> {
    let trimmed = text.trim();
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
