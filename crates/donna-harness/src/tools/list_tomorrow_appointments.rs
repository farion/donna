use donna_storage::LocalStore;

const DAY_SECONDS: i64 = 86_400;

pub(super) fn execute(store: &LocalStore) -> String {
    let now = now_seconds();
    let tomorrow_start = ((now / DAY_SECONDS) + 1) * DAY_SECONDS;
    let tomorrow_end = tomorrow_start + DAY_SECONDS;

    let events = match store.calendar_events_in_range(tomorrow_end, tomorrow_start, 25) {
        Ok(events) => events,
        Err(error) => return format!("I cannot read synced calendar events: {error}"),
    };

    if events.is_empty() {
        return "You have no synced appointments for tomorrow.".to_owned();
    }

    let mut answer = format!("Tomorrow you have {} appointment(s):", events.len());
    for event in events {
        let subject = event.subject.unwrap_or_else(|| "(no subject)".to_owned());
        let starts = event
            .starts_at
            .map(format_timestamp)
            .unwrap_or_else(|| "unknown start".to_owned());
        let ends = event
            .ends_at
            .map(format_timestamp)
            .unwrap_or_else(|| "unknown end".to_owned());
        answer.push_str("\n- ");
        answer.push_str(&subject);
        answer.push_str(" (");
        answer.push_str(&starts);
        answer.push_str(" - ");
        answer.push_str(&ends);
        answer.push(')');
    }

    answer
}

fn format_timestamp(seconds: i64) -> String {
    format!("unix:{seconds}")
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
