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
        let mut session = Self::new();
        session.push_donna_message(
            "Donna is running in local shell mode. Chat stays in memory for this session.",
        );
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
}
