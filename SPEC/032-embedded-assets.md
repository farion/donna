# Story 032: Embedded Assets

## User Story
As the user, I want Donna's built-in graphics shipped inside the binary, so that the app works without loose asset files.

## Acceptance Criteria
- Built-in avatar assets are embedded in the Rust binary.
- The `donna` character is available from embedded assets.
- Config selects a character by name.
- Only embedded character names are valid for v1.
- Missing configured characters fall back to `donna`.
- Missing image states fall back to `default.png`.
- The architecture allows adding more embedded characters later.

## Notes
- `rust-embed` is the recommended crate.
