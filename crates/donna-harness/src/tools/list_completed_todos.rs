use crate::tools::shared::format_completed_todo_list;
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore) -> String {
    match store.completed_todos(20) {
        Ok(todos) => format_completed_todo_list(&todos),
        Err(error) => format!("I cannot read your completed todos: {error}"),
    }
}
