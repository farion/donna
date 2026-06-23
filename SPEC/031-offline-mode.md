# Story 031: Offline Mode

## User Story
As the user, I want Donna to remain useful offline, so that I can still access local knowledge and tasks without network access.

## Acceptance Criteria
- Donna can open without network access.
- Local memories, todos, people, notes metadata, and synced messages remain searchable.
- Local Ollama can be used if configured and available.
- Graph sync failures are shown as stale data warnings.
- External send or calendar actions are not executed while offline.
- Approved external actions may be queued only if clearly shown and confirmed.

## Notes
- Offline behavior must be explicit to avoid accidental stale decisions.
