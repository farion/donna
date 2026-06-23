use crate::config::MemoryConfig;
use crate::storage::{LocalStore, NewMemory, NewPerson, NewTodo, StorageError};

const CHAT_SOURCE: &str = "donna_chat";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveMemoryApproval {
    RejectSensitive,
    ApproveSensitive,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MemoryExtraction {
    pub memories: Vec<NewMemory>,
    pub sensitive_memories: Vec<NewMemory>,
    pub todos: Vec<NewTodo>,
    pub people: Vec<NewPerson>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersistedExtraction {
    pub memory_ids: Vec<i64>,
    pub todo_ids: Vec<i64>,
    pub person_ids: Vec<i64>,
    pub skipped_sensitive: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryExtractor {
    require_sensitive_approval: bool,
}

impl MemoryExtractor {
    pub fn from_config(config: &MemoryConfig) -> Self {
        Self {
            require_sensitive_approval: config.require_sensitive_approval,
        }
    }

    pub fn extract_user_message(&self, message: &str) -> MemoryExtraction {
        let message = message.trim();
        if message.is_empty() {
            return MemoryExtraction::default();
        }

        let mut extraction = MemoryExtraction::default();

        if let Some(title) = extract_todo_title(message) {
            extraction.todos.push(NewTodo {
                title,
                notes: None,
                source: CHAT_SOURCE.to_owned(),
                related_topic: extract_topic(message),
                due_at: None,
            });
        }

        if let Some(meeting) = extract_meeting_memory(message) {
            if let Some(person) = extract_meeting_person(&meeting) {
                extraction.people.push(NewPerson {
                    display_name: person,
                    aliases: Vec::new(),
                    emails: Vec::new(),
                    teams_ids: Vec::new(),
                    context: Some("Mentioned in Donna chat meeting context".to_owned()),
                    source: CHAT_SOURCE.to_owned(),
                });
            }
            self.push_memory(
                &mut extraction,
                "meeting",
                format!("Meeting: {meeting}"),
                0.82,
                2,
            );
        }

        if let Some(preference) = extract_after(message, &["i prefer ", "my preference is "]) {
            self.push_memory(
                &mut extraction,
                "preference",
                format!("Preference: {}", clean_clause(preference)),
                0.8,
                1,
            );
        }

        if let Some(fact) = extract_after(message, &["remember that ", "fact: "]) {
            self.push_memory(
                &mut extraction,
                "fact",
                format!("Fact: {}", clean_clause(fact)),
                0.76,
                1,
            );
        }

        extraction
    }

    pub fn persist(
        &self,
        store: &LocalStore,
        extraction: &MemoryExtraction,
        approval: SensitiveMemoryApproval,
    ) -> Result<PersistedExtraction, StorageError> {
        let mut persisted = PersistedExtraction::default();

        for memory in &extraction.memories {
            persisted.memory_ids.push(store.create_memory(memory)?.id);
        }

        if approval == SensitiveMemoryApproval::ApproveSensitive {
            for memory in &extraction.sensitive_memories {
                persisted.memory_ids.push(store.create_memory(memory)?.id);
            }
        } else {
            persisted.skipped_sensitive = extraction.sensitive_memories.len();
        }

        for todo in &extraction.todos {
            persisted.todo_ids.push(store.create_todo(todo)?.id);
        }

        for person in &extraction.people {
            persisted.person_ids.push(store.create_person(person)?.id);
        }

        Ok(persisted)
    }

    fn push_memory(
        &self,
        extraction: &mut MemoryExtraction,
        memory_type: &str,
        content: String,
        confidence: f64,
        importance: i64,
    ) {
        let memory = NewMemory {
            memory_type: memory_type.to_owned(),
            content,
            source: CHAT_SOURCE.to_owned(),
            confidence,
            importance,
            expires_at: None,
        };

        if self.require_sensitive_approval && contains_sensitive_data(&memory.content) {
            extraction.sensitive_memories.push(memory);
        } else {
            extraction.memories.push(memory);
        }
    }
}

impl PersistedExtraction {
    pub fn has_records(&self) -> bool {
        !(self.memory_ids.is_empty() && self.todo_ids.is_empty() && self.person_ids.is_empty())
    }

    pub fn record_count(&self) -> usize {
        self.memory_ids.len() + self.todo_ids.len() + self.person_ids.len()
    }
}

fn extract_todo_title(message: &str) -> Option<String> {
    extract_after(
        message,
        &["todo:", "remember to ", "need to ", "must ", "have to "],
    )
    .map(clean_clause)
    .filter(|title| !title.is_empty())
}

fn extract_meeting_memory(message: &str) -> Option<String> {
    extract_after(message, &["meeting with ", "met with "])
        .map(clean_clause)
        .filter(|meeting| !meeting.is_empty())
}

fn extract_meeting_person(meeting: &str) -> Option<String> {
    let stop_words = [" about ", " for ", " on ", " regarding ", " and "];
    let mut end = meeting.len();
    let lower = meeting.to_lowercase();
    for word in stop_words {
        if let Some(index) = lower.find(word) {
            end = end.min(index);
        }
    }

    let candidate = meeting[..end].trim();
    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_owned())
    }
}

fn extract_topic(message: &str) -> Option<String> {
    extract_after(message, &[" about ", " regarding "])
        .map(clean_clause)
        .filter(|topic| !topic.is_empty())
}

fn extract_after<'a>(message: &'a str, patterns: &[&str]) -> Option<&'a str> {
    let lower = message.to_lowercase();
    patterns.iter().find_map(|pattern| {
        lower
            .find(pattern)
            .map(|index| &message[index + pattern.len()..])
    })
}

fn clean_clause(value: &str) -> String {
    let trimmed = value
        .split(['.', ';', '\n'])
        .next()
        .unwrap_or(value)
        .trim()
        .trim_matches('"')
        .trim_matches('\'');

    let mut clause = trimmed.to_owned();
    for delimiter in [" and must ", " and need to ", " but "] {
        if let Some(index) = clause.to_lowercase().find(delimiter) {
            clause.truncate(index);
        }
    }

    if clause.len() > 240 {
        clause.truncate(240);
        clause.push_str("...");
    }

    clause.trim().to_owned()
}

fn contains_sensitive_data(content: &str) -> bool {
    let lower = content.to_lowercase();
    [
        "password", "secret", "token", "ssn", "medical", "health", "salary",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::{MemoryExtractor, SensitiveMemoryApproval};
    use crate::config::MemoryConfig;
    use crate::storage::LocalStore;

    #[test]
    fn extracts_structured_records_without_raw_transcript() {
        let extractor = MemoryExtractor::from_config(&MemoryConfig::default());
        let raw = "I had a meeting with Anna about billing retries and must write a concept";

        let extraction = extractor.extract_user_message(raw);

        assert_eq!(extraction.todos[0].title, "write a concept");
        assert_eq!(extraction.memories[0].memory_type, "meeting");
        assert_eq!(extraction.memories[0].source, "donna_chat");
        assert_eq!(extraction.people[0].display_name, "Anna");
        assert_ne!(extraction.memories[0].content, raw);
        assert_ne!(extraction.todos[0].title, raw);
    }

    #[test]
    fn sensitive_memories_require_approval_before_persisting() {
        let extractor = MemoryExtractor::from_config(&MemoryConfig {
            require_sensitive_approval: true,
        });
        let store = LocalStore::in_memory().expect("store");
        let extraction = extractor.extract_user_message("remember that my password is swordfish");

        let persisted = extractor
            .persist(
                &store,
                &extraction,
                SensitiveMemoryApproval::RejectSensitive,
            )
            .expect("persist");

        assert_eq!(extraction.sensitive_memories.len(), 1);
        assert_eq!(persisted.memory_ids.len(), 0);
        assert_eq!(persisted.skipped_sensitive, 1);
    }

    #[test]
    fn persists_only_structured_memory_and_todo_records() {
        let extractor = MemoryExtractor::from_config(&MemoryConfig::default());
        let store = LocalStore::in_memory().expect("store");
        let extraction =
            extractor.extract_user_message("I prefer short updates and need to file receipts");

        let persisted = extractor
            .persist(
                &store,
                &extraction,
                SensitiveMemoryApproval::RejectSensitive,
            )
            .expect("persist");

        let memory = store.memory(persisted.memory_ids[0]).expect("memory");
        let todo = store.todo(persisted.todo_ids[0]).expect("todo");

        assert_eq!(memory.source, "donna_chat");
        assert_eq!(todo.source, "donna_chat");
        assert_ne!(
            memory.content,
            "I prefer short updates and need to file receipts"
        );
        assert_ne!(
            todo.title,
            "I prefer short updates and need to file receipts"
        );
    }
}
