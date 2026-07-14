use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, usize_argument};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let limit = usize_argument(&arguments, &["limit", "max"], 50);

    match store.list_teams_conversations(false, limit) {
        Ok(chats) if chats.is_empty() => "No synced Teams chats found.".to_owned(),
        Ok(chats) => {
            let mut output = format!("Found {} synced Teams chats:", chats.len());
            for chat in chats {
                output.push_str("\n- ");
                output.push_str(&chat);
            }
            output
        }
        Err(error) => format!("I cannot list synced Teams chats: {error}"),
    }
}
