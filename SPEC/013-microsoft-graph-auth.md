# Story 013: Microsoft Graph Auth

## User Story
As the user, I want Donna to authenticate with Microsoft Graph, so that it can access my Outlook, Teams, and calendar data.

## Acceptance Criteria
- Donna supports one Microsoft account at a time.
- Authentication uses OAuth device-code flow.
- Auth is started from `donna --auth`.
- Tokens are stored in OS secret storage.
- Non-secret account metadata is stored in TOML.
- Delegated permissions are used.
- Required scopes include user, offline access, mail, calendar, Teams chat, and Teams channel permissions.
- Donna detects and explains missing admin consent or unavailable Teams permissions.

## Notes
- Users may need their own Entra app registration or configured client id.
