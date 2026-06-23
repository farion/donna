#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedInput {
    Empty,
    Command(AppCommand),
    Message(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppCommand {
    Exit,
    Hide,
    Unknown(String),
}

pub fn parse_input(input: &str) -> ParsedInput {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return ParsedInput::Empty;
    }

    if let Some(command) = trimmed.strip_prefix('/') {
        let command_name = command.split_whitespace().next().unwrap_or_default();
        return ParsedInput::Command(match command_name {
            "exit" => AppCommand::Exit,
            "hide" => AppCommand::Hide,
            other => AppCommand::Unknown(other.to_owned()),
        });
    }

    ParsedInput::Message(trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{AppCommand, ParsedInput, parse_input};

    #[test]
    fn parses_known_commands() {
        assert_eq!(parse_input("/exit"), ParsedInput::Command(AppCommand::Exit));
        assert_eq!(
            parse_input(" /hide "),
            ParsedInput::Command(AppCommand::Hide)
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
            parse_input("/changechar donna"),
            ParsedInput::Command(AppCommand::Unknown("changechar".to_owned()))
        );
    }
}
