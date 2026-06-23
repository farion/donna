# Story 025: Pre-Meeting Briefings

## User Story
As the user, I want Donna to brief me before meetings, so that I know context, open todos, and relevant people before joining.

## Acceptance Criteria
- Donna can detect upcoming calendar events.
- Donna can generate a briefing before a meeting.
- Briefings may include attendees, previous messages, open todos, unresolved follow-ups, and known topics.
- Briefings are shown as attention events.
- Briefings use configured task prompts and task model.
- Donna does not send or modify anything during briefing generation.

## Notes
- Event-relative triggers may be needed in addition to cron.
