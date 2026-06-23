# Story 015: Teams Chat Sync And Send

## User Story
As the user, I want Donna to read and send Teams 1:1 and group chat messages, so that it can track work conversations and follow-ups.

## Acceptance Criteria
- Donna syncs personal and group Teams chats through Microsoft Graph.
- Donna persists synced Teams chat messages in SQLite.
- Stored records include message id, chat id, sender, timestamp, body, importance, URL where available, and sync timestamp.
- Donna can prepare replies.
- Sending Teams chat messages requires explicit approval.
- Sent actions are recorded in the audit log.
- Donna can detect waiting-for-me and waiting-for-them situations.

## Notes
- Teams chat support may depend on tenant permissions.
