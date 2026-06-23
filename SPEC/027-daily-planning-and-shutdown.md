# Story 027: Daily Planning And Shutdown

## User Story
As the user, I want Donna to help plan my day and close it down, so that I keep work organized.

## Acceptance Criteria
- Donna can run a morning planning task.
- Donna can run an evening shutdown task.
- Morning planning includes today's meetings, important todos, waiting replies, and conflicts.
- Evening shutdown includes completed items, open todos, deferred work, and optional diary capture.
- Both tasks are configured through task TOML and Markdown prompts.
- Findings are stored in SQLite.

## Notes
- These tasks should be useful without requiring note summarization by default.
