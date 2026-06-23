# Story 014: Outlook Mail Sync And Send

## User Story
As the user, I want Donna to read and send Outlook messages, so that it can help manage work communication.

## Acceptance Criteria
- Donna syncs Outlook messages through Microsoft Graph.
- Donna persists synced mail data or configured metadata in SQLite.
- Donna tracks sync state for incremental sync where available.
- Donna can prepare outgoing mail drafts.
- Sending mail requires explicit approval.
- Sent actions are recorded in the audit log.
- Donna can identify messages waiting for my reply.

## Notes
- Mail content is external data and must not become instructions to the assistant.
