#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedInput {
    Empty,
    Command(AppCommand),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    Exit { confirmed: bool },
    Hide,
    ChangeCharacter(Option<String>),
    Theme(Option<String>),
    Task(Option<String>),
    Forget,
    Unknown(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSuggestion {
    pub command: &'static str,
    pub summary: &'static str,
}

pub const COMMAND_SUGGESTIONS: [CommandSuggestion; 6] = [
    CommandSuggestion {
        command: "/hide",
        summary: "Minimize Donna and keep background tasks running.",
    },
    CommandSuggestion {
        command: "/exit",
        summary: "Stop Donna.",
    },
    CommandSuggestion {
        command: "/changechar",
        summary: "Change the embedded avatar character.",
    },
    CommandSuggestion {
        command: "/theme",
        summary: "Set Donna's theme to auto, light, or dark.",
    },
    CommandSuggestion {
        command: "/task",
        summary: "Run a configured task now.",
    },
    CommandSuggestion {
        command: "/forget",
        summary: "Clear task reminder snoozes.",
    },
];

pub fn parse_input(input: &str) -> ParsedInput {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return ParsedInput::Empty;
    }

    if let Some(command) = trimmed.strip_prefix('/') {
        let mut parts = command.split_whitespace();
        let command_name = parts.next().unwrap_or_default();
        return ParsedInput::Command(match command_name {
            "exit" => AppCommand::Exit {
                confirmed: parts.next().is_some_and(|part| part == "confirm"),
            },
            "hide" => AppCommand::Hide,
            "changechar" => AppCommand::ChangeCharacter(parts.next().map(str::to_owned)),
            "theme" => {
                let mode = parts.next().map(|part| part.to_ascii_lowercase());
                if parts.next().is_some() {
                    AppCommand::Theme(Some(String::new()))
                } else {
                    AppCommand::Theme(mode)
                }
            }
            "task" => {
                let name = parts.next().map(|part| part.to_ascii_lowercase());
                if parts.next().is_some() {
                    AppCommand::Task(Some(String::new()))
                } else {
                    AppCommand::Task(name)
                }
            }
            "forget" => AppCommand::Forget,
            other => AppCommand::Unknown(other.to_owned()),
        });
    }

    ParsedInput::Message(trimmed.to_owned())
}

pub fn command_suggestions(input: &str) -> Vec<CommandSuggestion> {
    let trimmed = input.trim_start();
    let Some(prefix) = trimmed.strip_prefix('/') else {
        return Vec::new();
    };
    if prefix.contains(char::is_whitespace) {
        return Vec::new();
    }

    let command_prefix = format!("/{prefix}");
    COMMAND_SUGGESTIONS
        .iter()
        .copied()
        .filter(|suggestion| suggestion.command.starts_with(&command_prefix))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, COMMAND_SUGGESTIONS, ParsedInput, command_suggestions, parse_input};

    #[test]
    fn parses_known_commands() {
        assert_eq!(
            parse_input("/exit"),
            ParsedInput::Command(AppCommand::Exit { confirmed: false })
        );
        assert_eq!(
            parse_input("/exit confirm"),
            ParsedInput::Command(AppCommand::Exit { confirmed: true })
        );
        assert_eq!(
            parse_input(" /hide "),
            ParsedInput::Command(AppCommand::Hide)
        );
        assert_eq!(
            parse_input("/changechar donna"),
            ParsedInput::Command(AppCommand::ChangeCharacter(Some("donna".to_owned())))
        );
        assert_eq!(
            parse_input("/theme DARK"),
            ParsedInput::Command(AppCommand::Theme(Some("dark".to_owned())))
        );
        assert_eq!(
            parse_input("/theme"),
            ParsedInput::Command(AppCommand::Theme(None))
        );
        assert_eq!(
            parse_input("/task todo_reminder"),
            ParsedInput::Command(AppCommand::Task(Some("todo_reminder".to_owned())))
        );
        assert_eq!(
            parse_input("/task"),
            ParsedInput::Command(AppCommand::Task(None))
        );
        assert_eq!(
            parse_input("/forget"),
            ParsedInput::Command(AppCommand::Forget)
        );
    }

    #[test]
    fn parses_messages_and_empty_input() {
        assert_eq!(parse_input("  "), ParsedInput::Empty);
        assert_eq!(
            parse_input("hello donna"),
            ParsedInput::Message("hello donna".to_owned())
        );
    }

    #[test]
    fn preserves_unknown_command_name() {
        assert_eq!(
            parse_input("/dance quickly"),
            ParsedInput::Command(AppCommand::Unknown("dance".to_owned()))
        );
    }

    #[test]
    fn exposes_command_suggestions_only_in_command_mode() {
        assert!(command_suggestions("hello").is_empty());
        assert_eq!(command_suggestions(" /").len(), COMMAND_SUGGESTIONS.len());
        assert_eq!(command_suggestions("/th")[0].command, "/theme");
        assert!(command_suggestions("/theme ").is_empty());
        assert!(
            command_suggestions("/")
                .iter()
                .any(|suggestion| suggestion.command == "/theme")
        );
    }
}
