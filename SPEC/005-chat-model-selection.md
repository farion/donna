# Story 005: Chat Model Selection

## User Story
As the user, I want to configure multiple LLMs and switch between them with Tab, so that I can choose the best model for the next chat message.

## Acceptance Criteria
- TOML config can define multiple models per provider.
- Each model has an id, label, provider, model name, base URL where needed, and optional secret reference.
- Pressing Tab cycles through configured chat models.
- The selected model label is shown top-right of the chat input bar.
- The selected model is persisted to config.
- Model changes do not interrupt an active AI response.
- The new selected model applies to the next submitted chat message.
- If the selected model is missing, Donna falls back to the first configured model.

## Notes
- Background tasks do not use the UI-selected model unless explicitly configured to do so.
