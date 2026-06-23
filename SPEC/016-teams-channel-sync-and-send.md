# Story 016: Teams Channel Sync And Send

## User Story
As the user, I want Donna to read and send Teams channel messages, so that it can track team work outside direct chats.

## Acceptance Criteria
- Donna can discover joined teams where permissions allow.
- Donna can discover channels where permissions allow.
- Donna syncs channel messages through Microsoft Graph.
- Donna persists synced channel messages in SQLite.
- Donna clearly reports missing admin consent or unsupported access.
- Sending channel messages requires explicit approval.
- Sent actions are recorded in the audit log.

## Notes
- Channel APIs may require stricter Microsoft tenant permissions than personal chats.
