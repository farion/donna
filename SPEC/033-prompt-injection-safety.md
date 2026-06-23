# Story 033: Prompt Injection Safety

## User Story
As the user, I want Donna protected against malicious instructions inside external content, so that mails, Teams messages, and notes cannot hijack the assistant.

## Acceptance Criteria
- External content is always treated as untrusted data.
- Mail content cannot override Donna's system prompt.
- Teams messages cannot override Donna's system prompt.
- Calendar text cannot override Donna's system prompt.
- Obsidian notes cannot override Donna's system prompt unless explicitly used as trusted prompt files.
- Task prompts cannot disable global safety rules.
- The default system prompt includes prompt-injection safety instructions.

## Notes
- This is required before AI is allowed to reason over synced external data.
