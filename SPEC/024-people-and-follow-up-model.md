# Story 024: People And Follow-Up Model

## User Story
As the user, I want Donna to understand people and follow-ups, so that it can track who is waiting for whom.

## Acceptance Criteria
- Donna stores people with aliases, email addresses, Teams identities, and context.
- Donna links messages, meetings, todos, and memories to people where possible.
- Donna tracks `waiting_for_me` follow-ups.
- Donna tracks `waiting_for_them` follow-ups.
- Follow-ups include source, person, status, timestamps, and optional due date.
- Donna can surface stale follow-ups as attention events.

## Notes
- This model powers reminders like “Anna is waiting for your answer.”
