use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, string_argument};

pub(super) fn execute(call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let subject = string_argument(&arguments, &["subject", "title"]);
    let starts_at = string_argument(&arguments, &["starts_at", "start"]);
    let ends_at = string_argument(&arguments, &["ends_at", "end"]);

    if subject.is_none() || starts_at.is_none() || ends_at.is_none() {
        return "To create an appointment, provide subject, starts_at, and ends_at.".to_owned();
    }

    "Calendar create requires explicit approval and is not model-executed directly in this tool path yet.".to_owned()
}
