# Story 020: Background Cron Task Runner

## User Story
As the user, I want Donna to run scheduled background tasks, so that it proactively organizes my work-life.

## Acceptance Criteria
- Task schedules use cron expressions.
- The task runner evaluates enabled tasks regularly.
- Task runs are recorded in SQLite.
- Task findings are recorded in SQLite.
- Background tasks use `[ai.tasks].default_model` unless the task overrides it.
- If no usable task model exists, AI-backed tasks are disabled with a visible config error.
- `/exit` stops the task runner.

## Notes
- Some future tasks may need event-relative triggers in addition to cron.
