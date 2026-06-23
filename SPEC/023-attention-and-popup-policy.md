# Story 023: Attention And Popup Policy

## User Story
As the user, I want Donna to pop up for important attention events, so that it actively organizes me even when I hide it.

## Acceptance Criteria
- Donna supports attention levels such as info, normal, important, and critical.
- Donna can show notifications for lower attention levels.
- Donna can pop up its window for important or critical events.
- Donna can pop up after `/hide`.
- Attention behavior is configurable.
- Donna uses `attention.png` when surfacing reminders, todos, collisions, or follow-ups.
- Wayland limitations are handled gracefully when focus stealing is not allowed.

## Notes
- Donna is intentionally proactive and dominant, but platform rules may limit forced focus.
