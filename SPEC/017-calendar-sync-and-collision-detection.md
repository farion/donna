# Story 017: Calendar Sync And Collision Detection

## User Story
As the user, I want Donna to sync my calendar and detect appointment collisions, so that I can avoid scheduling conflicts.

## Acceptance Criteria
- Donna reads calendar events through Microsoft Graph.
- Donna stores calendar events in SQLite.
- Event times are normalized to UTC while preserving timezone metadata.
- Cancelled events are ignored for collision detection.
- Free or transparent events are ignored.
- Busy, tentative, and out-of-office overlaps are detected.
- Collisions are stored as task findings.
- Calendar changes require explicit approval.

## Notes
- Relevant Graph endpoints include `/me/calendarView` and `/me/events`.
