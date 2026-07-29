use crate::tools::ModelToolCall;
use crate::tools::shared::{
    normalized_call_arguments, optional_i64_argument, string_argument, usize_argument,
};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall) -> String {
    let arguments = normalized_call_arguments(call);
    let title = string_argument(&arguments, &["title", "subject"]);
    let text = string_argument(&arguments, &["text", "query"]);
    let person = string_argument(&arguments, &["person", "sender"]);
    let after = optional_i64_argument(&arguments, &["date_from", "from", "after"]).flatten();
    let before = optional_i64_argument(&arguments, &["date_to", "to", "before"]).flatten();
    let limit = usize_argument(&arguments, &["limit", "max"], 20);

    match store.search_outlook_messages(
        title.as_deref(),
        text.as_deref(),
        person.as_deref(),
        after,
        before,
        limit,
    ) {
        Ok(messages) if messages.is_empty() => "No synced Outlook mails matched your filters.".to_owned(),
        Ok(messages) => {
            if messages.len() == 1 {
                return format_exact_mail(&messages[0]);
            }
            let mut output = format!("Found {} matching synced Outlook mails:", messages.len());
            for mail in messages {
                output.push_str("\n- ");
                output.push_str(mail.subject.as_deref().unwrap_or("(no subject)"));
                if let Some(sender) = mail.sender_name.or(mail.sender_email) {
                    output.push_str(" from ");
                    output.push_str(&sender);
                }
            }
            output
        }
        Err(error) => format!("I cannot search synced Outlook mails: {error}"),
    }
}

fn format_exact_mail(mail: &donna_storage::OutlookMessage) -> String {
    let mut output = String::from("Found 1 synced Outlook mail:");
    if let Some(sender) = mail.sender_name.as_deref().or(mail.sender_email.as_deref()) {
        output.push_str("\n- From: ");
        output.push_str(sender);
    }
    if let Some(subject) = mail.subject.as_deref() {
        output.push_str("\n- Subject: ");
        output.push_str(subject);
    }
    if let Some(body_preview) = mail.body_preview.as_deref() {
        output.push_str("\n- Body: ");
        output.push_str(body_preview);
    }
    if let Some(received_at) = mail.received_at {
        output.push_str("\n- Received: ");
        output.push_str(&donna_core::time::format_unix_timestamp_human(received_at));
    }
    output
}
