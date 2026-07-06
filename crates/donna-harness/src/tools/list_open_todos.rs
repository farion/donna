use crate::tools::shared::format_open_todo_list;
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore) -> String {
    match store.open_todos(20) {
        Ok(todos) => format_open_todo_list(&todos),
        Err(error) => format!("I cannot read your todos: {error}"),
    }
}
