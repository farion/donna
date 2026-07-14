use crate::tools::ModelToolCall;
use crate::tools::shared::{
    normalized_call_arguments, optional_i64_argument, string_argument, usize_argument,
};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let text = string_argument(&arguments, &["text", "query"]);
    let person = string_argument(&arguments, &["person", "sender"]);
    let after = optional_i64_argument(&arguments, &["date_from", "from", "after"]).flatten();
    let before = optional_i64_argument(&arguments, &["date_to", "to", "before"]).flatten();
    let limit = usize_argument(&arguments, &["limit", "max"], 25);

    match store.search_teams_messages(text.as_deref(), person.as_deref(), after, before, limit) {
        Ok(messages) if messages.is_empty() => "No synced Teams messages matched your filters.".to_owned(),
        Ok(messages) => {
            let mut output = format!("Found {} matching synced Teams messages:", messages.len());
            for message in messages {
                output.push_str("\n- ");
                output.push_str(message.sender_name.as_deref().unwrap_or("(unknown sender)"));
                output.push_str(": ");
                output.push_str(&truncate(&message.body, 120));
            }
            output
        }
        Err(error) => format!("I cannot search synced Teams messages: {error}"),
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut output = text.chars().take(max.saturating_sub(3)).collect::<String>();
    output.push_str("...");
    output
}
