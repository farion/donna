use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, string_argument};

pub(super) fn execute(call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let conversation = string_argument(&arguments, &["chat_id", "channel_id", "conversation"]);
    let body = string_argument(&arguments, &["body", "text"]);

    if conversation.is_none() || body.is_none() {
        return "To send a Teams message, provide conversation id and body.".to_owned();
    }

    "Teams send requires explicit approval and is not model-executed directly in this tool path yet.".to_owned()
}
