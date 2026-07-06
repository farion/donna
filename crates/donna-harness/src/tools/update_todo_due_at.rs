use crate::tools::ModelToolCall;
use crate::tools::shared::{i64_argument, normalized_call_arguments, optional_i64_argument};
use donna_storage::LocalStore;

#[derive(Debug)]
struct UpdateTodoDueAtArgs {
    todo_id: i64,
    due_at: Option<i64>,
}

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    match UpdateTodoDueAtArgs::from_call(call) {
        Ok(args) => match store.update_todo_due_at(args.todo_id, args.due_at) {
            Ok(todo) => match todo.due_at {
                Some(due_at) => format!("Set '{}' due_at to {}.", todo.title, due_at),
                None => format!("Cleared due_at for '{}'.", todo.title),
            },
            Err(error) => format!("I could not update that todo: {error}"),
        },
        Err(error) => format!("I could not read that todo tool call: {error}"),
    }
}

impl UpdateTodoDueAtArgs {
    fn from_call(call: &ModelToolCall) -> Result<Self, String> {
        let arguments = normalized_call_arguments(call);
        let todo_id = i64_argument(&arguments, &["todo_id", "id"])
            .ok_or_else(|| "missing todo_id".to_owned())?;
        let due_at = optional_i64_argument(&arguments, &["due_at", "due", "due_date"])
            .ok_or_else(|| "missing due_at".to_owned())?;
        Ok(Self { todo_id, due_at })
    }
}
