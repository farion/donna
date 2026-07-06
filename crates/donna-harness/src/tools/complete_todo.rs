use crate::tools::ModelToolCall;
use crate::tools::shared::{i64_argument, normalized_call_arguments};
use donna_storage::LocalStore;

#[derive(Debug)]
struct CompleteTodoArgs {
    todo_id: i64,
}

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    match CompleteTodoArgs::from_call(call) {
        Ok(args) => match store.update_todo_status(args.todo_id, "done") {
            Ok(todo) => format!("Marked '{}' done.", todo.title),
            Err(error) => format!("I could not complete that todo: {error}"),
        },
        Err(error) => format!("I could not read that todo tool call: {error}"),
    }
}

impl CompleteTodoArgs {
    fn from_call(call: &ModelToolCall) -> Result<Self, String> {
        let arguments = normalized_call_arguments(call);
        let todo_id = i64_argument(&arguments, &["todo_id", "id"])
            .ok_or_else(|| "missing todo_id".to_owned())?;
        Ok(Self { todo_id })
    }
}
