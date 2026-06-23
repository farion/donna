# Story 003: Avatar State Images

## User Story
As the user, I want Donna's avatar to change expression based on state, so that I can understand what the assistant is doing at a glance.

## Acceptance Criteria
- The configured character determines the avatar identity.
- Initial character is `donna`.
- Avatar assets are embedded in the Rust binary.
- Assets are addressed conceptually as `assets/characters/{character}/`.
- Supported files are `default.png`, `idle-1.png`, `idle-2.png`, `idle-3.png`, `attention.png`, `question.png`, `thinking.png`, and `command.png`.
- Missing state images fall back to `default.png`.
- Unknown characters fall back to `donna`.
- Idle frames are shown randomly for less than one second.
- Idle animation does not interrupt thinking, question, attention, or command states.

## Notes
- Use an embedded asset strategy such as `rust-embed`.
