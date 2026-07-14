use crate::tools::ModelToolCall;
use crate::tools::shared::{
    normalized_call_arguments, optional_i64_argument, string_argument, usize_argument,
};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let text = string_argument(&arguments, &["text", "query", "title"]);
    let people = string_argument(&arguments, &["persons", "people", "organizer"]);
    let after = optional_i64_argument(&arguments, &["date_from", "from", "after"]).flatten();
    let before = optional_i64_argument(&arguments, &["date_to", "to", "before"]).flatten();
    let limit = usize_argument(&arguments, &["limit", "max"], 25);

    match store.search_calendar_events(text.as_deref(), people.as_deref(), after, before, limit) {
        Ok(events) if events.is_empty() => "No synced appointments matched your filters.".to_owned(),
        Ok(events) => {
            let mut output = format!("Found {} matching synced appointments:", events.len());
            for event in events {
                output.push_str("\n- ");
                output.push_str(event.subject.as_deref().unwrap_or("(no subject)"));
            }
            output
        }
        Err(error) => format!("I cannot search synced appointments: {error}"),
    }
}
