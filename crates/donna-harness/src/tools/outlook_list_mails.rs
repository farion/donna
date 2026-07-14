use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, optional_i64_argument, usize_argument};
use donna_storage::LocalStore;

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall, user_message: &str) -> String {
    let arguments = normalized_call_arguments(call);
    let explicit_date_filter = mentions_date_filter(user_message);
    let after = explicit_date_filter
        .then(|| optional_i64_argument(&arguments, &["date_from", "from", "after"]).flatten())
        .flatten();
    let before = explicit_date_filter
        .then(|| optional_i64_argument(&arguments, &["date_to", "to", "before"]).flatten())
        .flatten();
    let limit = usize_argument(&arguments, &["limit", "max"], 20);

    match store.list_outlook_messages(after, before, limit) {
        Ok(messages) if messages.is_empty() => "No synced Outlook mails matched.".to_owned(),
        Ok(messages) => {
            if messages.len() == 1 {
                return format_exact_mail(&messages[0]);
            }
            let mut output = format!("Found {} synced Outlook mails:", messages.len());
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
        Err(error) => format!("I cannot list synced Outlook mails: {error}"),
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
        output.push_str("\n- ReceivedAt: ");
        output.push_str(&received_at.to_string());
    }
    output
}

fn mentions_date_filter(user_message: &str) -> bool {
    let lower = user_message.to_ascii_lowercase();
    [
        "today",
        "yesterday",
        "this week",
        "last week",
        "this month",
        "last month",
        "between",
        "before",
        "after",
        "since",
        "from ",
        "to ",
        "on ",
        "date",
    ]
    .into_iter()
    .any(|needle| lower.contains(needle))
}
