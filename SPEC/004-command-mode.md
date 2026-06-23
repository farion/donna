# Story 004: Command Mode

## User Story
As the user, I want slash commands in Donna's chat input, so that I can control the app quickly.

## Acceptance Criteria
- Command mode starts when the input begins with `/`.
- Donna uses the `command.png` avatar state in command mode.
- `/exit` exits the app completely, including background tasks.
- `/hide` hides the whole Donna window.
- Donna may pop up again after `/hide` for attention-worthy events.
- `/changechar [name]` changes the avatar character.
- Character changes are persisted to `~/.config/donna/donna.toml`.
- Unknown commands show an ephemeral error.
- Commands are not persisted as Donna chat history.

## Notes
- Command handling must not be interpreted by the AI provider.
