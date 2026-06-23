# Story 028: Local Search And Retrieval

## User Story
As the user, I want Donna to search local work data, so that it can answer questions using my messages, calendar, notes, todos, and memories.

## Acceptance Criteria
- Donna can search memories, todos, people, Teams messages, Outlook messages, calendar events, task findings, and notes metadata.
- SQLite FTS5 is used or planned for text retrieval.
- Search supports source, date, person, status, and topic filters where possible.
- Search results distinguish external content from trusted instructions.
- Search works offline over locally synced data.

## Notes
- Vector embeddings are optional future work, not required for v1.
