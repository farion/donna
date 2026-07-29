use crate::tools::ModelToolCall;
use crate::tools::shared::{
    event_id_argument, format_appointment_line, format_single_event_by_id,
    normalized_call_arguments, optional_i64_argument, usize_argument, wants_attendees,
};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall, user_message: &str) -> String {
    let arguments = normalized_call_arguments(call);
    let include_attendees = wants_attendees(user_message);

    if let Some(event_id) = event_id_argument(&arguments) {
        return format_single_event_by_id(store, event_id, include_attendees);
    }

    let after = optional_i64_argument(&arguments, &["date_from", "from", "after"]).flatten();
    let before = optional_i64_argument(&arguments, &["date_to", "to", "before"]).flatten();
    let limit = usize_argument(&arguments, &["limit", "max"], 25);

    match store.list_calendar_events(after, before, limit) {
        Ok(events) if events.is_empty() => "No synced calendar appointments matched.".to_owned(),
        Ok(events) => {
            let mut output = format!("Found {} synced appointments:", events.len());
            for event in events {
                output.push_str("\n- ");
                output.push_str(&format_appointment_line(&event, include_attendees));
            }
            output
        }
        Err(error) => format!("I cannot list synced appointments: {error}"),
    }
}
