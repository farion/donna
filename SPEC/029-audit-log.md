# Story 029: Audit Log

## User Story
As the user, I want Donna to keep an audit log of external actions, so that I can understand what it changed or sent.

## Acceptance Criteria
- Donna records approved outgoing mail actions.
- Donna records approved Teams send actions.
- Donna records approved calendar mutations.
- Donna records approved note writes or edits.
- Audit entries include action type, target system, summary, approval timestamp, execution timestamp, result, and external id where available.
- Audit logs do not include secrets.
- Audit logs avoid raw Donna chat transcripts.

## Notes
- Audit logs are local SQLite records.
