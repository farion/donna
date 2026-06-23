# Story 018: Task Config Folder

## User Story
As the user, I want background tasks defined in a folder, so that I can customize Donna's proactive behavior.

## Acceptance Criteria
- Task directory is configured in TOML.
- Default task directory is `~/.config/donna/tasks`.
- Each task has a TOML metadata file.
- Each task may reference a Markdown prompt file.
- Task metadata includes id, enabled state, kind, cron schedule, prompt file, and optional model override.
- Disabled tasks are not executed.
- Invalid task configs are reported clearly.

## Notes
- V1 tasks map to built-in Rust task kinds, not arbitrary shell scripts.
