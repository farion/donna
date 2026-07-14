use crate::tools::ModelToolCall;
use donna_storage::StoredTodo;

pub(super) fn normalized_call_arguments(
    call: &ModelToolCall,
) -> serde_json::Map<String, serde_json::Value> {
    if let Some(arguments) = object_from_value(&call.arguments) {
        return arguments;
    }

    call.extra
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "tool" | "name" | "function"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(super) fn i64_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<i64> {
    names.iter().find_map(|name| match arguments.get(*name)? {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    })
}

pub(super) fn optional_i64_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<Option<i64>> {
    names.iter().find_map(|name| match arguments.get(*name)? {
        serde_json::Value::Null => Some(None),
        serde_json::Value::Number(number) => number.as_i64().map(Some),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok().map(Some),
        _ => None,
    })
}

pub(super) fn usize_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
    default: usize,
) -> usize {
    names
        .iter()
        .find_map(|name| match arguments.get(*name)? {
            serde_json::Value::Number(number) => number.as_u64().map(|value| value as usize),
            serde_json::Value::String(text) => text.trim().parse::<usize>().ok(),
            _ => None,
        })
        .unwrap_or(default)
}

pub(super) fn string_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<String> {
    names.iter().find_map(|name| match arguments.get(*name)? {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        _ => None,
    })
}

pub(super) fn format_open_todo_list(todos: &[StoredTodo]) -> String {
    if todos.is_empty() {
        return "You have no open todos.".to_owned();
    }
    if todos.len() == 1 {
        return format!(
            "You have one open todo: {}. It is {} priority.",
            todos[0].title, todos[0].severity
        );
    }

    let mut answer = format!("You have {} open todos:\n", todos.len());
    for todo in todos {
        answer.push_str("- ");
        answer.push_str(&todo.title);
        answer.push_str(" (");
        answer.push_str(&todo.severity);
        answer.push_str(" priority)\n");
    }
    answer.trim_end().to_owned()
}

pub(super) fn format_completed_todo_list(todos: &[StoredTodo]) -> String {
    if todos.is_empty() {
        return "You have no completed todos.".to_owned();
    }
    if todos.len() == 1 {
        return format!("You have one completed todo: {}.", todos[0].title);
    }

    let mut answer = format!("You have {} completed todos:\n", todos.len());
    for todo in todos {
        answer.push_str("- ");
        answer.push_str(&todo.title);
        answer.push('\n');
    }
    answer.trim_end().to_owned()
}

fn object_from_value(
    value: &serde_json::Value,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match value {
        serde_json::Value::Object(object) if !object.is_empty() => Some(object.clone()),
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| object_from_value(&value)),
        _ => None,
    }
}
