use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, string_argument};

pub(super) fn execute(call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let to = string_argument(&arguments, &["to", "recipient"]);
    let subject = string_argument(&arguments, &["subject", "title"]);
    let body = string_argument(&arguments, &["body", "text"]);

    if to.is_none() || subject.is_none() || body.is_none() {
        return "To send Outlook mail, provide to, subject, and body.".to_owned();
    }

    "Outlook send requires explicit approval and is not model-executed directly in this tool path yet.".to_owned()
}
