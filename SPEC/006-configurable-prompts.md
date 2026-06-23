# Story 006: Configurable Prompts

## User Story
As the user, I want Donna's system prompt and task prompts to be configurable, so that I can tune behavior without recompiling the app.

## Acceptance Criteria
- The interactive Donna system prompt is loaded from a configured Markdown file.
- If no prompt file exists, Donna uses an embedded default system prompt.
- If embedded prompt loading fails, Donna uses a minimal hardcoded fallback.
- Task prompts are Markdown files referenced by task TOML files.
- System prompts and task prompts are trusted only from built-ins or local user config.
- External content from mail, Teams, calendar, or notes cannot override system instructions.

## Notes
- Default path: `~/.config/donna/prompts/system.md`.
