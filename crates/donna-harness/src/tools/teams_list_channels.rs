use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, usize_argument};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let limit = usize_argument(&arguments, &["limit", "max"], 50);

    match store.list_teams_conversations(true, limit) {
        Ok(channels) if channels.is_empty() => "No synced Teams channels found.".to_owned(),
        Ok(channels) => {
            let mut output = format!("Found {} synced Teams channels:", channels.len());
            for channel in channels {
                output.push_str("\n- ");
                output.push_str(&channel);
            }
            output
        }
        Err(error) => format!("I cannot list synced Teams channels: {error}"),
    }
}
