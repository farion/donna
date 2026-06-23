# Story 030: Sync State And Delta Sync

## User Story
As the user, I want Donna to sync efficiently and reliably, so that local data stays current without repeated full downloads.

## Acceptance Criteria
- Donna stores sync state per external source.
- Sync state includes source, cursor or delta link where available, last sync timestamp, and error state.
- Synced records store external ids and etags or change keys where available.
- Deleted external records are marked deleted locally instead of silently disappearing.
- Microsoft Graph delta queries are used where suitable.
- Full resync can be triggered when sync state is invalid.

## Notes
- Sync errors should be visible but not fatal to the whole app.
