use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, string_argument};
use donna_storage::LocalStore;

const LOOKBACK_DAYS: i64 = 90;
const DAY_SECONDS: i64 = 86_400;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let Some(person) = string_argument(&arguments, &["person", "name", "contact"]) else {
        return "Tell me the person name to summarize (for example: Alex Example).".to_owned();
    };

    let cutoff = now_seconds() - (LOOKBACK_DAYS * DAY_SECONDS);
    let messages = match store.recent_teams_messages_from_sender(&person, cutoff, 12) {
        Ok(messages) => messages,
        Err(error) => return format!("I cannot read synced Teams messages: {error}"),
    };

    if messages.is_empty() {
        return format!(
            "I have no synced Teams messages from {person} in the last {LOOKBACK_DAYS} days."
        );
    }

    let mut response = format!(
        "Here is a short summary of your recent Teams discussion with {person}:"
    );
    for message in messages.iter().take(5) {
        let body = compact_text(&message.body);
        if body.is_empty() {
            continue;
        }
        response.push_str("\n- ");
        response.push_str(&truncate(&body, 140));
    }
    response
}

fn compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut out = text.chars().take(max_chars.saturating_sub(1)).collect::<String>();
    out.push_str("...");
    out
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
