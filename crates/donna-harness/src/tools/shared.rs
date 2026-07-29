use crate::tools::ModelToolCall;
use donna_storage::{CalendarEvent, LocalStore, StoredTodo};

const DAY_SECONDS: i64 = 86_400;

/// The `(start, end)` Unix-second bounds of a single local day, relative to
/// today (0 = today, 1 = tomorrow, ...), computed from the real system clock.
/// Shared by every place that needs to replace a model-guessed date range
/// with the real boundary for "today"/"tomorrow" instead of trusting the
/// model's own date arithmetic.
pub(super) fn day_bounds_for_offset(days_from_today: i64) -> (i64, i64) {
    let now = now_seconds();
    let day_start = (now.div_euclid(DAY_SECONDS) + days_from_today) * DAY_SECONDS;
    (day_start, day_start + DAY_SECONDS)
}

/// Formats synced calendar events for a single day relative to today (0 =
/// today, 1 = tomorrow, ...), computed from the real system clock rather
/// than a date the model supplies — this is what keeps "today"/"tomorrow"
/// questions correct even when the model has no reliable notion of "now".
/// When `only_first` is set (the user asked for their "first"/"next"
/// appointment on that day, not the full list), only the earliest event is
/// reported.
pub(super) fn format_day_appointments(
    store: &LocalStore,
    days_from_today: i64,
    day_label: &str,
    only_first: bool,
    remaining_only: bool,
    include_attendees: bool,
) -> String {
    let now = now_seconds();
    let (day_start, day_end) = day_bounds_for_offset(days_from_today);
    let range_start = if remaining_only {
        day_start.max(now)
    } else {
        day_start
    };

    let events = match store.calendar_events_in_range(day_end, range_start, 25) {
        Ok(events) => events,
        Err(error) => return format!("I cannot read synced calendar events: {error}"),
    };

    if events.is_empty() {
        let scope = if remaining_only { "left for" } else { "for" };
        return format!("You have no synced appointments {scope} {day_label}.");
    }

    if only_first {
        let first = &events[0];
        return format!(
            "Your first meeting {day_label}: {}",
            format_appointment_line(first, include_attendees)
        );
    }

    let mut answer = format!(
        "{}{} you have {} appointment(s){}:",
        day_label[..1].to_uppercase(),
        &day_label[1..],
        events.len(),
        if remaining_only { " left" } else { "" },
    );
    for event in &events {
        answer.push_str("\n- ");
        answer.push_str(&format_appointment_line(event, include_attendees));
    }

    answer
}

/// True when the user's own message asks for a single specific appointment
/// on a day ("first"/"next" meeting) rather than the full day's list.
pub(super) fn wants_single_appointment(user_message: &str) -> bool {
    let question = user_message.to_ascii_lowercase();
    question.contains("first meeting")
        || question.contains("first appointment")
        || question.contains("next meeting")
        || question.contains("next appointment")
}

/// True when the user asked for what's still ahead today/tomorrow rather
/// than the full day, e.g. "what's left today" or "remaining meetings" —
/// past appointments for the day should be excluded from the answer.
pub(super) fn wants_remaining_only(user_message: &str) -> bool {
    let question = user_message.to_ascii_lowercase();
    question.contains("left today")
        || question.contains("left for today")
        || question.contains("remaining today")
        || question.contains("rest of today")
        || question.contains("still today")
        || question.contains("meetings left")
        || question.contains("appointments left")
        || question.contains("what's left")
        || question.contains("whats left")
}

/// True only when the user explicitly asked about attendees/participants —
/// keeps the default appointment listing terse instead of always dumping
/// every attendee's name for every meeting.
pub(super) fn wants_attendees(user_message: &str) -> bool {
    let question = user_message.to_ascii_lowercase();
    question.contains("attendee")
        || question.contains("participant")
        || question.contains("invitee")
        || question.contains("who is")
        || question.contains("who's")
        || question.contains("who are")
        || question.contains("who will")
}

/// Finds the single soonest upcoming synced calendar event (starting now or
/// later), computed from the real system clock rather than a date the model
/// supplies. This is what answers "what's my next meeting?" correctly,
/// instead of the model guessing a zero-width date_from/date_to range on
/// `calendar_list_appointments` that can only ever match an event starting
/// at that exact second. When `person` is given (e.g. "next meeting with
/// Alex"), it narrows to the soonest upcoming event matching that
/// organizer/attendee instead of the very next event overall.
pub(super) fn format_next_appointment(
    store: &LocalStore,
    include_attendees: bool,
    person: Option<&str>,
) -> String {
    let now = now_seconds();
    let events = match person {
        Some(person) => store.search_calendar_events(None, Some(person), Some(now), None, 25),
        None => store.list_calendar_events(Some(now), None, 10),
    };
    let events = match events {
        Ok(events) => events,
        Err(error) => return format!("I cannot read synced calendar events: {error}"),
    };

    match events.iter().find(|event| is_blocking_event(event)) {
        Some(event) => format!(
            "Your next meeting{}: {}",
            person
                .map(|person| format!(" with {person}"))
                .unwrap_or_default(),
            format_appointment_line(event, include_attendees)
        ),
        None => match person {
            Some(person) => format!("You have no upcoming synced appointments with {person}."),
            None => "You have no upcoming synced appointments.".to_owned(),
        },
    }
}

/// True for events that actually occupy the calendar as a real meeting.
/// Excludes all-day entries (out-of-office, parental leave, multi-day status
/// blocks use Graph's `isAllDay` flag regardless of their `show_as` value —
/// "Elternzeit" showed up as `show_as: "tentative"`, so `show_as` alone
/// can't distinguish a real meeting from a whole-day/multi-day block).
/// Also excludes events explicitly marked "free", matching
/// `calendar_collisions`' filter.
fn is_blocking_event(event: &CalendarEvent) -> bool {
    if event.is_all_day {
        return false;
    }
    match event.show_as.as_deref() {
        Some(show_as) => matches!(
            show_as.to_ascii_lowercase().as_str(),
            "busy" | "tentative" | "oof"
        ),
        None => true,
    }
}

/// Formats a single calendar event as "Subject (start - end) [event_id: N]"
/// with human-readable local timestamps — just the time for something
/// happening today (e.g. "Standup (09:00 - 09:30)"), or "Wed Jul 29, 09:00"
/// for another day — for consistent output across every calendar tool. The
/// trailing event_id is a stable handle the model can pass back on a
/// follow-up call (e.g. "who's attending that meeting?") to fetch this exact
/// event again instead of re-deriving a text/date/person filter from memory,
/// which local models are unreliable at and which can match the wrong event
/// when several share the same day or attendee.
pub(super) fn format_appointment_line(event: &CalendarEvent, include_attendees: bool) -> String {
    let subject = event.subject.as_deref().unwrap_or("(no subject)");
    let starts = event
        .starts_at
        .map(donna_core::time::format_unix_timestamp_human)
        .unwrap_or_else(|| "unknown start".to_owned());
    let ends = event
        .ends_at
        .map(donna_core::time::format_unix_timestamp_human)
        .unwrap_or_else(|| "unknown end".to_owned());
    let mut line = format!("{subject} ({starts} - {ends}) [event_id: {}]", event.id);
    if include_attendees {
        line.push_str(", attendees: ");
        line.push_str(&format_attendees(&event.attendees));
    }
    line
}

/// Always states the attendees explicitly, including "none" — the model has
/// no other source for this fact, so leaving the clause out when the list is
/// empty reads as missing data and the model fabricates or refuses to answer
/// a follow-up like "who's attending?" instead of saying there's no one else.
fn format_attendees(attendees: &[donna_storage::CalendarAttendee]) -> String {
    if attendees.is_empty() {
        return "none listed".to_owned();
    }
    attendees
        .iter()
        .map(|attendee| {
            attendee
                .name
                .as_deref()
                .or(attendee.email.as_deref())
                .unwrap_or("(unknown)")
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Reads the `event_id` a prior tool result surfaced back from the model,
/// letting a follow-up call fetch that exact event directly instead of
/// re-deriving a text/date/person filter — see `format_appointment_line`.
pub(super) fn event_id_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
) -> Option<i64> {
    i64_argument(arguments, &["event_id", "id"])
}

/// Extracts the event_id from a prior Donna reply, but only when exactly one
/// `[event_id: N]` tag appears in it — more than one tag means that reply
/// covered several appointments, so there is no single unambiguous "that
/// meeting" for a follow-up to fall back to. Used as a deterministic safety
/// net when the model's own follow-up call omits `event_id` (local models are
/// unreliable at carrying a structured reference across turns on their own).
pub(super) fn referenced_event_id(text: &str) -> Option<i64> {
    let mut ids = event_id_tags(text);
    let first = ids.next()?;
    if ids.next().is_some() { None } else { Some(first) }
}

const EVENT_ID_TAG_PREFIX: &str = "[event_id: ";

fn event_id_tags(text: &str) -> impl Iterator<Item = i64> + '_ {
    text.match_indices(EVENT_ID_TAG_PREFIX).filter_map(|(start, _)| {
        let after = &text[start + EVENT_ID_TAG_PREFIX.len()..];
        let end = after.find(']')?;
        after[..end].trim().parse::<i64>().ok()
    })
}

/// Finds the single line in a calendar tool result whose "start - end" time
/// window also appears in `answer_text`, and returns its event_id. Used when
/// a tool result listed several candidate appointments but the model's own
/// answer settled on one by quoting its time — the case left once
/// `referenced_event_id` gives up on an ambiguous multi-tag result. The time
/// window is taken as the last "(...)" group before the `[event_id: N]` tag
/// rather than the line's first "(", since a meeting subject can itself
/// contain parentheses (e.g. "Feature Ready (Sprint 1) (10:00 - 11:30)"), and
/// compared with its surrounding parens stripped, since a natural-language
/// answer typically drops them ("...scheduled for 10:00 - 11:30" rather than
/// "...scheduled for (10:00 - 11:30)").
pub(super) fn matching_event_id_by_time(answer_text: &str, raw_tool_result: &str) -> Option<i64> {
    let mut matches = raw_tool_result.lines().filter_map(|line| {
        let tag_start = line.find(EVENT_ID_TAG_PREFIX)?;
        let before_tag = line[..tag_start].trim_end();
        if !before_tag.ends_with(')') {
            return None;
        }
        let open = before_tag.rfind('(')?;
        let window = &before_tag[open + 1..before_tag.len() - 1];
        if !answer_text.contains(window) {
            return None;
        }
        let after = &line[tag_start + EVENT_ID_TAG_PREFIX.len()..];
        let end = after.find(']')?;
        after[..end].trim().parse::<i64>().ok()
    });
    let first = matches.next()?;
    if matches.next().is_some() { None } else { Some(first) }
}

/// Fetches and formats a single synced event by its local id, for the
/// `event_id` fast path on the calendar list/search tools.
pub(super) fn format_single_event_by_id(
    store: &LocalStore,
    event_id: i64,
    include_attendees: bool,
) -> String {
    match store.calendar_event_by_id(event_id) {
        Ok(Some(event)) => format_appointment_line(&event, include_attendees),
        Ok(None) => format!("I cannot find a synced appointment with event_id {event_id}."),
        Err(error) => format!("I cannot read synced calendar events: {error}"),
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub(super) fn normalized_call_arguments(
    call: &ModelToolCall,
) -> serde_json::Map<String, serde_json::Value> {
    if let Some(arguments) = object_from_value(&call.arguments) {
        return arguments;
    }

    call.extra
        .iter()
        .filter(|(key, _)| !matches!(key.as_str(), "tool" | "name" | "function"))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub(super) fn i64_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<i64> {
    names.iter().find_map(|name| match arguments.get(*name)? {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok(),
        _ => None,
    })
}

pub(super) fn optional_i64_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<Option<i64>> {
    names.iter().find_map(|name| match arguments.get(*name)? {
        serde_json::Value::Null => Some(None),
        serde_json::Value::Number(number) => number.as_i64().map(Some),
        serde_json::Value::String(text) => text.trim().parse::<i64>().ok().map(Some),
        _ => None,
    })
}

pub(super) fn usize_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
    default: usize,
) -> usize {
    names
        .iter()
        .find_map(|name| match arguments.get(*name)? {
            serde_json::Value::Number(number) => number.as_u64().map(|value| value as usize),
            serde_json::Value::String(text) => text.trim().parse::<usize>().ok(),
            _ => None,
        })
        .unwrap_or(default)
}

pub(super) fn string_argument(
    arguments: &serde_json::Map<String, serde_json::Value>,
    names: &[&str],
) -> Option<String> {
    names.iter().find_map(|name| match arguments.get(*name)? {
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_owned())
        }
        _ => None,
    })
}

pub(super) fn format_open_todo_list(todos: &[StoredTodo]) -> String {
    if todos.is_empty() {
        return "You have no open todos.".to_owned();
    }
    if todos.len() == 1 {
        return format!(
            "You have one open todo: {}. It is {} priority.",
            todos[0].title, todos[0].severity
        );
    }

    let mut answer = format!("You have {} open todos:\n", todos.len());
    for todo in todos {
        answer.push_str("- ");
        answer.push_str(&todo.title);
        answer.push_str(" (");
        answer.push_str(&todo.severity);
        answer.push_str(" priority)\n");
    }
    answer.trim_end().to_owned()
}

pub(super) fn format_completed_todo_list(todos: &[StoredTodo]) -> String {
    if todos.is_empty() {
        return "You have no completed todos.".to_owned();
    }
    if todos.len() == 1 {
        return format!("You have one completed todo: {}.", todos[0].title);
    }

    let mut answer = format!("You have {} completed todos:\n", todos.len());
    for todo in todos {
        answer.push_str("- ");
        answer.push_str(&todo.title);
        answer.push('\n');
    }
    answer.trim_end().to_owned()
}

fn object_from_value(
    value: &serde_json::Value,
) -> Option<serde_json::Map<String, serde_json::Value>> {
    match value {
        serde_json::Value::Object(object) if !object.is_empty() => Some(object.clone()),
        serde_json::Value::String(text) => serde_json::from_str::<serde_json::Value>(text)
            .ok()
            .and_then(|value| object_from_value(&value)),
        _ => None,
    }
}
