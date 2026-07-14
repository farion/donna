use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, string_argument};

pub(super) fn execute(call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let appointment_id = string_argument(&arguments, &["appointment_id", "external_id", "id"]);

    if appointment_id.is_none() {
        return "To delete an appointment, provide appointment_id.".to_owned();
    }

    "Calendar delete requires explicit approval and is not model-executed directly in this tool path yet.".to_owned()
}
