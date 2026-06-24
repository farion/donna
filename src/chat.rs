use std::time::{SystemTime, UNIX_EPOCH};

const WELCOME_MESSAGES: [&str; 5] = [
    "Donna is warmed up, dangerous in heels, and ready to make your day behave.",
    "Systems awake. Tell me what needs taming, darling.",
    "I am online, sharp, and just a little trouble. What are we conquering first?",
    "Donna is here: polished, wickedly capable, and waiting for your next move.",
    "Booted, focused, and dressed to ruin chaos. Give me something worth handling.",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speaker {
    Donna,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: u64,
    pub speaker: Speaker,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct ChatSession {
    messages: Vec<ChatMessage>,
    next_id: u64,
}

impl ChatSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_welcome() -> Self {
        Self::with_welcome_message(random_welcome_message())
    }

    pub fn with_welcome_message(message: impl Into<String>) -> Self {
        let mut session = Self::new();
        session.push_donna_message(message);
        session
    }

    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }

    pub fn push_user_message(&mut self, text: impl Into<String>) -> Option<u64> {
        self.push_message(Speaker::User, text)
    }

    pub fn push_donna_message(&mut self, text: impl Into<String>) -> Option<u64> {
        self.push_message(Speaker::Donna, text)
    }

    pub fn replace_message_text(&mut self, id: u64, text: impl Into<String>) -> bool {
        let Some(message) = self.messages.iter_mut().find(|message| message.id == id) else {
            return false;
        };
        message.text = text.into();
        true
    }

    fn push_message(&mut self, speaker: Speaker, text: impl Into<String>) -> Option<u64> {
        let text = text.into();
        let text = text.trim();

        if text.is_empty() {
            return None;
        }

        let id = self.next_id;
        self.next_id += 1;
        self.messages.push(ChatMessage {
            id,
            speaker,
            text: text.to_owned(),
        });
        Some(id)
    }
}

fn random_welcome_message() -> &'static str {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.subsec_nanos() as usize)
        .unwrap_or(0);
    WELCOME_MESSAGES[nanos % WELCOME_MESSAGES.len()]
}

#[cfg(test)]
mod tests {
    use super::{ChatSession, Speaker};

    #[test]
    fn stores_messages_only_in_the_session() {
        let mut session = ChatSession::new();

        assert_eq!(session.push_user_message(" hello "), Some(0));
        assert_eq!(session.push_donna_message("hi"), Some(1));

        let messages = session.messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].speaker, Speaker::User);
        assert_eq!(messages[0].text, "hello");
        assert_eq!(messages[1].speaker, Speaker::Donna);
    }

    #[test]
    fn ignores_empty_messages_without_allocating_ids() {
        let mut session = ChatSession::new();

        assert_eq!(session.push_user_message("  "), None);
        assert_eq!(session.push_donna_message("ready"), Some(0));
        assert_eq!(session.messages().len(), 1);
    }

    #[test]
    fn replaces_existing_message_text() {
        let mut session = ChatSession::new();
        let id = session.push_donna_message("thinking").expect("id");

        assert!(session.replace_message_text(id, "done"));
        assert_eq!(session.messages()[0].text, "done");
        assert!(!session.replace_message_text(99, "missing"));
    }

    #[test]
    fn welcome_message_is_flirty_without_local_shell_stub() {
        let session = ChatSession::with_welcome();
        let message = &session.messages()[0].text;

        assert!(!message.contains("local shell"));
        assert!(super::WELCOME_MESSAGES.contains(&message.as_str()));
    }

    #[test]
    fn can_start_with_custom_welcome_message() {
        let session = ChatSession::with_welcome_message("Hello Frieder.");

        assert_eq!(session.messages()[0].text, "Hello Frieder.");
    }
}
