# Story 007: TOML Configuration

## User Story
As the user, I want Donna configured through TOML, so that settings are transparent and easy to edit.

## Acceptance Criteria
- Default config path on Linux is `~/.config/donna/donna.toml`.
- Config stores non-secret settings only.
- Config includes UI, avatar, AI models, Microsoft Graph, notes, task, memory, and attention settings.
- Config references secrets by keyring reference names.
- Donna creates a default config when none exists.
- Invalid config produces clear errors and safe fallback behavior where possible.

## Notes
- Config writes must preserve user intent and avoid destroying unrelated settings.
