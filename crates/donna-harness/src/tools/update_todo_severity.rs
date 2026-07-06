use crate::tools::ModelToolCall;
use crate::tools::shared::{i64_argument, normalized_call_arguments, string_argument};
use donna_storage::LocalStore;

#[derive(Debug)]
struct UpdateTodoSeverityArgs {
    todo_id: i64,
    severity: String,
}

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    match UpdateTodoSeverityArgs::from_call(call) {
        Ok(args) => match store.update_todo_severity(args.todo_id, &args.severity) {
            Ok(todo) => format!("Set '{}' to {} priority.", todo.title, todo.severity),
            Err(error) => format!("I could not update that todo: {error}"),
        },
        Err(error) => format!("I could not read that todo tool call: {error}"),
    }
}

impl UpdateTodoSeverityArgs {
    fn from_call(call: &ModelToolCall) -> Result<Self, String> {
        let arguments = normalized_call_arguments(call);
        let todo_id = i64_argument(&arguments, &["todo_id", "id"])
            .ok_or_else(|| "missing todo_id".to_owned())?;
        let severity = string_argument(&arguments, &["severity", "priority"])
            .ok_or_else(|| "missing severity".to_owned())?;
        Ok(Self { todo_id, severity })
    }
}
