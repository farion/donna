use crate::tools::ModelToolCall;
use crate::tools::shared::{normalized_call_arguments, optional_i64_argument, string_argument};
use donna_storage::{LocalStore, NewTodo};

#[derive(Debug)]
struct CreateTodoArgs {
    title: String,
    severity: Option<String>,
    notes: Option<String>,
    related_topic: Option<String>,
    due_at: Option<i64>,
}

pub(super) fn execute(store: &LocalStore, call: &ModelToolCall, user_message: &str) -> String {
    match CreateTodoArgs::from_call(call) {
        Ok(args) if args.title.trim().is_empty() => {
            "I could not create that todo because it had no title.".to_owned()
        }
        Ok(args) if !title_is_grounded_in_user_message(&args.title, user_message) => {
            "I did not add that todo because it was not in your message.".to_owned()
        }
        Ok(args) => match store.create_todo(&NewTodo {
            title: args.title.trim().to_owned(),
            notes: args.notes,
            source: "donna_chat".to_owned(),
            related_topic: args.related_topic,
            severity: args.severity.unwrap_or_else(|| "middle".to_owned()),
            due_at: args.due_at,
        }) {
            Ok(todo) => format!("Added todo: {}.", todo.title),
            Err(error) => format!("I could not add that todo: {error}"),
        },
        Err(error) => format!("I could not read that todo tool call: {error}"),
    }
}

impl CreateTodoArgs {
    fn from_call(call: &ModelToolCall) -> Result<Self, String> {
        let arguments = normalized_call_arguments(call);
        let title = string_argument(&arguments, &["title", "task", "todo"])
            .ok_or_else(|| "missing title".to_owned())?;
        Ok(Self {
            title,
            severity: string_argument(&arguments, &["severity", "priority"]),
            notes: string_argument(&arguments, &["notes", "note"]),
            related_topic: string_argument(&arguments, &["related_topic", "topic"]),
            due_at: optional_i64_argument(&arguments, &["due_at", "due", "due_date"]).flatten(),
        })
    }
}

fn title_is_grounded_in_user_message(title: &str, user_message: &str) -> bool {
    let title_words = significant_words(title);
    if title_words.is_empty() {
        return false;
    }
    let message_words = significant_words(user_message);

    title_words.iter().all(|word| message_words.contains(word))
}

fn significant_words(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .map(str::trim)
        .filter(|word| word.len() > 2)
        .map(str::to_ascii_lowercase)
        .filter(|word| {
            !matches!(
                word.as_str(),
                "the"
                    | "and"
                    | "that"
                    | "this"
                    | "todo"
                    | "task"
                    | "done"
                    | "must"
                    | "today"
                    | "tomorrow"
                    | "soon"
                    | "urgent"
                    | "important"
                    | "priority"
                    | "add"
                    | "new"
            )
        })
        .collect()
}
