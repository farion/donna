# Story 001: Ephemeral Chat And Memory Extraction

## User Story
As the user, I want to chat with Donna without storing raw chat transcripts, so that I can use the assistant privately while still letting it remember relevant work-life facts.

## Acceptance Criteria
- Donna chat messages are kept in memory only for the active session.
- Raw Donna chat messages are not written to SQLite, logs, task history, or prompt history.
- Donna auto-detects relevant durable information from chat.
- Donna may persist structured memories, todos, people, preferences, and meeting notes.
- Persisted records include source `donna_chat`, timestamp, confidence, and type.
- Sensitive memories require explicit approval before persistence.
- The user can review, correct, and delete stored memories.

## Notes
- Example: “I had a meeting with Anna about billing retries and must write a concept” becomes a meeting memory and todo, not a stored raw chat line.
