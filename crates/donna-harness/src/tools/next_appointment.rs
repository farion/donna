use crate::tools::ModelToolCall;
use crate::tools::shared::{
    format_next_appointment, normalized_call_arguments, string_argument, wants_attendees,
};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall, user_message: &str) -> String {
    let arguments = normalized_call_arguments(call);
    let person = string_argument(&arguments, &["persons", "people", "person", "organizer"]);
    format_next_appointment(store, wants_attendees(user_message), person.as_deref())
}
