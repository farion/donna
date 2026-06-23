# Story 011: SQLite Local Store

## User Story
As the user, I want Donna to store work data locally in SQLite, so that my personal assistant state is private and available offline.

## Acceptance Criteria
- SQLite stores Teams messages, Outlook messages, calendar events, todos, memories, people, task findings, sync state, and audit logs.
- SQLite does not store raw Donna chat transcripts.
- Schema changes are migration-controlled.
- The database path is configurable.
- FTS5 is used or planned for local search.
- Records include source and timestamps where relevant.
- Sync metadata supports external ids, etags or change keys, and deletion state.

## Notes
- Database encryption is not required for v1 but must be documented as a future option.
