# Story 019: Markdown Task Prompts

## User Story
As the user, I want task behavior described with Markdown prompts, so that I can tune Donna's reasoning and wording.

## Acceptance Criteria
- Task prompts are Markdown files.
- Task TOML references the prompt file.
- Built-in task prompts can be embedded in the binary.
- User task prompts can live in `~/.config/donna/tasks`.
- Task prompts do not execute code.
- Task prompts cannot override global safety rules.
- Missing prompts use built-in defaults where available.

## Notes
- Prompt files are instructions for AI reasoning, not plugin code.
