# Story 008: OS Secret Storage

## User Story
As the user, I want secrets stored in the operating system secret store, so that API keys and tokens are not written to project files.

## Acceptance Criteria
- AI API keys are stored in OS secret storage.
- Microsoft tokens are stored in OS secret storage.
- TOML config stores only secret references.
- Linux uses the platform secret service where available, such as libsecret or KWallet-backed storage.
- Windows uses Windows Credential Manager through the selected library.
- macOS uses Keychain through the selected library.
- Missing secrets trigger an auth/config prompt instead of crashing.

## Notes
- Prefer the Rust `keyring` crate as the cross-platform abstraction.
