use crate::tools::shared::{
    format_day_appointments, wants_attendees, wants_remaining_only, wants_single_appointment,
};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, user_message: &str) -> String {
    format_day_appointments(
        store,
        0,
        "today",
        wants_single_appointment(user_message),
        wants_remaining_only(user_message),
        wants_attendees(user_message),
    )
}
