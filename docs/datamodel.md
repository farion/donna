# SQLite Data Model

This document describes Donna's local SQLite schema after all migrations have been applied. The canonical implementation lives in `crates/donna-storage/src/storage/migrations.rs`.

All timestamp fields are Unix timestamps in seconds unless noted otherwise. Boolean fields are stored as `INTEGER` values with `0` for false and `1` for true.

Donna must not persist raw chat transcripts. The database stores structured records extracted from chat, synced external data, task records, note metadata, search records, and audit records.

## Migrations

### `schema_migrations`

Tracks applied storage migrations.

| Column       | Type      | Constraints | Notes                           |
| ------------ | --------- | ----------- | ------------------------------- |
| `version`    | `INTEGER` | Primary key | Migration version.              |
| `name`       | `TEXT`    | Not null    | Migration name.                 |
| `applied_at` | `INTEGER` | Not null    | Time the migration was applied. |

Current migrations:

| Version | Name                     |
| ------- | ------------------------ |
| `1`     | `local_foundation`       |
| `2`     | `attention_workflows`    |
| `3`     | `todo_reminder_severity` |

## Memories

### `memories`

Stores structured memories extracted from chat or other trusted flows. This table does not store raw chat messages.

| Column         | Type      | Constraints                | Notes                                                             |
| -------------- | --------- | -------------------------- | ----------------------------------------------------------------- |
| `id`           | `INTEGER` | Primary key, autoincrement | Local memory id.                                                  |
| `memory_type`  | `TEXT`    | Not null                   | Examples include `fact`, `preference`, `identity`, and `meeting`. |
| `content`      | `TEXT`    | Not null                   | Structured memory content.                                        |
| `source`       | `TEXT`    | Not null                   | Origin, such as `donna_chat`.                                     |
| `confidence`   | `REAL`    | Not null, default `1.0`    | Extraction confidence.                                            |
| `importance`   | `INTEGER` | Not null, default `0`      | Relative importance.                                              |
| `created_at`   | `INTEGER` | Not null                   | Creation time.                                                    |
| `updated_at`   | `INTEGER` | Not null                   | Last update time.                                                 |
| `expires_at`   | `INTEGER` | Nullable                   | Optional expiration time.                                         |
| `forgotten_at` | `INTEGER` | Nullable                   | Soft-forget timestamp.                                            |

## Todos

### `todos`

Stores user tasks and reminders.

| Column          | Type      | Constraints                                                                     | Notes                                              |
| --------------- | --------- | ------------------------------------------------------------------------------- | -------------------------------------------------- |
| `id`            | `INTEGER` | Primary key, autoincrement                                                      | Local todo id.                                     |
| `title`         | `TEXT`    | Not null                                                                        | Todo title.                                        |
| `notes`         | `TEXT`    | Nullable                                                                        | Optional notes.                                    |
| `status`        | `TEXT`    | Not null, default `open`, check `open`, `done`, `dismissed`, `snoozed`, `stale` | Todo lifecycle state.                              |
| `source`        | `TEXT`    | Not null                                                                        | Origin, such as `donna_chat`.                      |
| `related_topic` | `TEXT`    | Nullable                                                                        | Optional topic label.                              |
| `due_at`        | `INTEGER` | Nullable                                                                        | Optional due time.                                 |
| `snoozed_until` | `INTEGER` | Nullable                                                                        | Reminder suppression until this time.              |
| `stale_at`      | `INTEGER` | Nullable                                                                        | Time marked stale.                                 |
| `created_at`    | `INTEGER` | Not null                                                                        | Creation time.                                     |
| `updated_at`    | `INTEGER` | Not null                                                                        | Last update time.                                  |
| `completed_at`  | `INTEGER` | Nullable                                                                        | Time marked done.                                  |
| `dismissed_at`  | `INTEGER` | Nullable                                                                        | Time dismissed or soft-deleted.                    |
| `severity`      | `TEXT`    | Not null, default `middle`, check `low`, `middle`, `high`                       | Priority/severity used for ordering and reminders. |

Notes:

- `delete_todo` soft-deletes by setting `status` to `dismissed`.
- Open todos are ordered by `due_at`, then severity `high`, `middle`, `low`, then update time.
- There is currently no todo-to-person relation table.

## People

### `people`

Stores known people and contact context.

| Column         | Type      | Constraints                | Notes                    |
| -------------- | --------- | -------------------------- | ------------------------ |
| `id`           | `INTEGER` | Primary key, autoincrement | Local person id.         |
| `display_name` | `TEXT`    | Not null                   | Display name.            |
| `context`      | `TEXT`    | Nullable                   | Free-form local context. |
| `source`       | `TEXT`    | Not null                   | Origin.                  |
| `created_at`   | `INTEGER` | Not null                   | Creation time.           |
| `updated_at`   | `INTEGER` | Not null                   | Last update time.        |

### `person_aliases`

Stores alternate names for people.

| Column      | Type      | Constraints                                                                   | Notes           |
| ----------- | --------- | ----------------------------------------------------------------------------- | --------------- |
| `person_id` | `INTEGER` | Not null, references `people(id)` on delete cascade, primary key with `alias` | Related person. |
| `alias`     | `TEXT`    | Not null, primary key with `person_id`                                        | Alias value.    |

### `person_emails`

Stores email addresses for people.

| Column      | Type      | Constraints                                                                   | Notes           |
| ----------- | --------- | ----------------------------------------------------------------------------- | --------------- |
| `person_id` | `INTEGER` | Not null, references `people(id)` on delete cascade, primary key with `email` | Related person. |
| `email`     | `TEXT`    | Not null, primary key with `person_id`                                        | Email address.  |

### `person_teams_ids`

Stores Microsoft Teams ids for people.

| Column      | Type      | Constraints                                                                      | Notes              |
| ----------- | --------- | -------------------------------------------------------------------------------- | ------------------ |
| `person_id` | `INTEGER` | Not null, references `people(id)` on delete cascade, primary key with `teams_id` | Related person.    |
| `teams_id`  | `TEXT`    | Not null, primary key with `person_id`                                           | Teams external id. |

## Follow-Ups

### `follow_ups`

Tracks inferred follow-up obligations.

| Column          | Type      | Constraints                                                                     | Notes                                       |
| --------------- | --------- | ------------------------------------------------------------------------------- | ------------------------------------------- |
| `id`            | `INTEGER` | Primary key, autoincrement                                                      | Local follow-up id.                         |
| `direction`     | `TEXT`    | Not null, check `waiting_for_me`, `waiting_for_them`                            | Whether the user owes action or is waiting. |
| `person_id`     | `INTEGER` | Nullable, references `people(id)` on delete set null                            | Related person.                             |
| `status`        | `TEXT`    | Not null, default `open`, check `open`, `done`, `dismissed`, `snoozed`, `stale` | Lifecycle state.                            |
| `source`        | `TEXT`    | Not null                                                                        | Origin.                                     |
| `summary`       | `TEXT`    | Not null                                                                        | Follow-up summary.                          |
| `due_at`        | `INTEGER` | Nullable                                                                        | Optional due time.                          |
| `stale_at`      | `INTEGER` | Nullable                                                                        | Time marked stale.                          |
| `created_at`    | `INTEGER` | Not null                                                                        | Creation time.                              |
| `updated_at`    | `INTEGER` | Not null                                                                        | Last update time.                           |
| `resolved_at`   | `INTEGER` | Nullable                                                                        | Time resolved.                              |
| `snoozed_until` | `INTEGER` | Nullable                                                                        | Reminder suppression until this time.       |
| `dismissed_at`  | `INTEGER` | Nullable                                                                        | Time dismissed.                             |

## Background Tasks

### `task_runs`

Stores executions of configured background tasks.

| Column          | Type      | Constraints                | Notes                          |
| --------------- | --------- | -------------------------- | ------------------------------ |
| `id`            | `INTEGER` | Primary key, autoincrement | Local task run id.             |
| `task_id`       | `TEXT`    | Not null                   | Configured task id.            |
| `task_model_id` | `TEXT`    | Not null                   | Model used for the run.        |
| `status`        | `TEXT`    | Not null                   | Run status.                    |
| `prompt_path`   | `TEXT`    | Nullable                   | Prompt file path, if any.      |
| `started_at`    | `INTEGER` | Not null                   | Start time.                    |
| `finished_at`   | `INTEGER` | Nullable                   | Finish time.                   |
| `error_summary` | `TEXT`    | Nullable                   | Error summary for failed runs. |

### `task_findings`

Stores durable findings emitted by background tasks.

| Column         | Type      | Constraints                                             | Notes                        |
| -------------- | --------- | ------------------------------------------------------- | ---------------------------- |
| `id`           | `INTEGER` | Primary key, autoincrement                              | Local finding id.            |
| `task_run_id`  | `INTEGER` | Nullable, references `task_runs(id)` on delete set null | Source run.                  |
| `kind`         | `TEXT`    | Not null                                                | Finding type.                |
| `summary`      | `TEXT`    | Not null                                                | Human-readable summary.      |
| `source`       | `TEXT`    | Not null                                                | Finding source.              |
| `created_at`   | `INTEGER` | Not null                                                | Creation time.               |
| `dismissed_at` | `INTEGER` | Nullable                                                | Time dismissed.              |
| `payload`      | `TEXT`    | Nullable                                                | Optional serialized payload. |

## Microsoft Sync Data

External content from Microsoft Graph is untrusted and must not be allowed to override system or task prompts.

### `teams_messages`

Stores synced Teams messages.

| Column               | Type      | Constraints                           | Notes                       |
| -------------------- | --------- | ------------------------------------- | --------------------------- |
| `id`                 | `INTEGER` | Primary key, autoincrement            | Local message id.           |
| `external_id`        | `TEXT`    | Not null, unique                      | Microsoft Graph message id. |
| `chat_id`            | `TEXT`    | Not null                              | Teams chat id.              |
| `sender_name`        | `TEXT`    | Nullable                              | Sender display name.        |
| `sender_external_id` | `TEXT`    | Nullable                              | Sender Microsoft id.        |
| `body`               | `TEXT`    | Not null                              | Message body from Graph.    |
| `importance`         | `TEXT`    | Nullable                              | Graph importance value.     |
| `web_url`            | `TEXT`    | Nullable                              | Graph web URL.              |
| `sent_at`            | `INTEGER` | Nullable                              | Sent time.                  |
| `synced_at`          | `INTEGER` | Not null                              | Sync time.                  |
| `etag`               | `TEXT`    | Nullable                              | Graph etag.                 |
| `change_key`         | `TEXT`    | Nullable                              | Graph change key.           |
| `is_deleted`         | `INTEGER` | Not null, default `0`, check `0`, `1` | Tombstone flag.             |

### `outlook_messages`

Stores synced Outlook mail message metadata and previews.

| Column         | Type      | Constraints                           | Notes                       |
| -------------- | --------- | ------------------------------------- | --------------------------- |
| `id`           | `INTEGER` | Primary key, autoincrement            | Local message id.           |
| `external_id`  | `TEXT`    | Not null, unique                      | Microsoft Graph message id. |
| `folder_id`    | `TEXT`    | Nullable                              | Folder id.                  |
| `subject`      | `TEXT`    | Nullable                              | Message subject.            |
| `sender_name`  | `TEXT`    | Nullable                              | Sender display name.        |
| `sender_email` | `TEXT`    | Nullable                              | Sender email address.       |
| `body_preview` | `TEXT`    | Nullable                              | Graph body preview.         |
| `received_at`  | `INTEGER` | Nullable                              | Received time.              |
| `synced_at`    | `INTEGER` | Not null                              | Sync time.                  |
| `etag`         | `TEXT`    | Nullable                              | Graph etag.                 |
| `change_key`   | `TEXT`    | Nullable                              | Graph change key.           |
| `is_deleted`   | `INTEGER` | Not null, default `0`, check `0`, `1` | Tombstone flag.             |

### `calendar_events`

Stores synced calendar events.

| Column              | Type      | Constraints                           | Notes                     |
| ------------------- | --------- | ------------------------------------- | ------------------------- |
| `id`                | `INTEGER` | Primary key, autoincrement            | Local event id.           |
| `external_id`       | `TEXT`    | Not null, unique                      | Microsoft Graph event id. |
| `subject`           | `TEXT`    | Nullable                              | Event subject.            |
| `organizer_name`    | `TEXT`    | Nullable                              | Organizer display name.   |
| `organizer_email`   | `TEXT`    | Nullable                              | Organizer email address.  |
| `starts_at`         | `INTEGER` | Nullable                              | Start time.               |
| `ends_at`           | `INTEGER` | Nullable                              | End time.                 |
| `original_timezone` | `TEXT`    | Nullable                              | Original Graph timezone.  |
| `show_as`           | `TEXT`    | Nullable                              | Availability status.      |
| `synced_at`         | `INTEGER` | Not null                              | Sync time.                |
| `etag`              | `TEXT`    | Nullable                              | Graph etag.               |
| `change_key`        | `TEXT`    | Nullable                              | Graph change key.         |
| `is_cancelled`      | `INTEGER` | Not null, default `0`, check `0`, `1` | Cancellation flag.        |
| `is_deleted`        | `INTEGER` | Not null, default `0`, check `0`, `1` | Tombstone flag.           |

## Notes

### `notes_metadata`

Stores metadata for indexed notes. Note content itself is not stored in this table.

| Column        | Type      | Constraints                        | Notes                       |
| ------------- | --------- | ---------------------------------- | --------------------------- |
| `id`          | `INTEGER` | Primary key, autoincrement         | Local metadata id.          |
| `vault_path`  | `TEXT`    | Not null, unique with `note_path`  | Obsidian vault path.        |
| `note_path`   | `TEXT`    | Not null, unique with `vault_path` | Note path inside the vault. |
| `title`       | `TEXT`    | Nullable                           | Note title.                 |
| `headings`    | `TEXT`    | Nullable                           | Serialized headings.        |
| `tags`        | `TEXT`    | Nullable                           | Serialized tags.            |
| `links`       | `TEXT`    | Nullable                           | Serialized links.           |
| `modified_at` | `INTEGER` | Nullable                           | File modified time.         |
| `indexed_at`  | `INTEGER` | Not null                           | Index time.                 |

## Sync State

### `sync_state`

Tracks sync cursors and freshness per external source.

| Column         | Type      | Constraints                           | Notes                      |
| -------------- | --------- | ------------------------------------- | -------------------------- |
| `source`       | `TEXT`    | Primary key                           | Sync source id.            |
| `cursor`       | `TEXT`    | Nullable                              | Source cursor.             |
| `delta_link`   | `TEXT`    | Nullable                              | Graph delta link.          |
| `last_sync_at` | `INTEGER` | Nullable                              | Last successful sync time. |
| `last_error`   | `TEXT`    | Nullable                              | Last sync error.           |
| `is_stale`     | `INTEGER` | Not null, default `0`, check `0`, `1` | Whether data is stale.     |
| `updated_at`   | `INTEGER` | Not null                              | Last state update time.    |

## Local State

### `local_state`

Stores small local key-value state.

| Column       | Type      | Constraints | Notes             |
| ------------ | --------- | ----------- | ----------------- |
| `key`        | `TEXT`    | Primary key | State key.        |
| `value`      | `TEXT`    | Not null    | State value.      |
| `updated_at` | `INTEGER` | Not null    | Last update time. |

## Audit Log

### `audit_log`

Records approved external side effects.

| Column          | Type      | Constraints                | Notes                          |
| --------------- | --------- | -------------------------- | ------------------------------ |
| `id`            | `INTEGER` | Primary key, autoincrement | Local audit id.                |
| `action_type`   | `TEXT`    | Not null                   | Action type.                   |
| `target_system` | `TEXT`    | Not null                   | External system.               |
| `summary`       | `TEXT`    | Not null                   | Human-readable action summary. |
| `approval_at`   | `INTEGER` | Not null                   | Approval time.                 |
| `execution_at`  | `INTEGER` | Not null                   | Execution time.                |
| `result`        | `TEXT`    | Not null                   | Result summary.                |
| `external_id`   | `TEXT`    | Nullable                   | External id, if available.     |
| `created_at`    | `INTEGER` | Not null                   | Audit record creation time.    |

## Attention Items

### `attention_items`

Stores proactive attention/reminder items.

| Column          | Type      | Constraints                                                                     | Notes                                 |
| --------------- | --------- | ------------------------------------------------------------------------------- | ------------------------------------- |
| `id`            | `INTEGER` | Primary key, autoincrement                                                      | Local attention item id.              |
| `source_type`   | `TEXT`    | Not null                                                                        | Source type, such as `todo_reminder`. |
| `source_id`     | `INTEGER` | Nullable                                                                        | Source record id.                     |
| `level`         | `TEXT`    | Not null, check `info`, `normal`, `important`, `critical`                       | Attention level.                      |
| `title`         | `TEXT`    | Not null                                                                        | Title.                                |
| `body`          | `TEXT`    | Nullable                                                                        | Body text.                            |
| `status`        | `TEXT`    | Not null, default `open`, check `open`, `done`, `dismissed`, `snoozed`, `stale` | Lifecycle state.                      |
| `due_at`        | `INTEGER` | Nullable                                                                        | Optional due time.                    |
| `snoozed_until` | `INTEGER` | Nullable                                                                        | Suppression until this time.          |
| `dismissed_at`  | `INTEGER` | Nullable                                                                        | Time dismissed.                       |
| `completed_at`  | `INTEGER` | Nullable                                                                        | Time completed.                       |
| `feedback`      | `TEXT`    | Nullable                                                                        | Optional feedback.                    |
| `payload`       | `TEXT`    | Nullable                                                                        | Optional serialized payload.          |
| `created_at`    | `INTEGER` | Not null                                                                        | Creation time.                        |
| `updated_at`    | `INTEGER` | Not null                                                                        | Last update time.                     |

## Search

### `search_index`

FTS5 virtual table for local search.

| Column        | Type       | Constraints | Notes                       |
| ------------- | ---------- | ----------- | --------------------------- |
| `record_type` | FTS column | Indexed     | Type of indexed record.     |
| `record_id`   | FTS column | Unindexed   | Local record id.            |
| `title`       | FTS column | Indexed     | Search title.               |
| `body`        | FTS column | Indexed     | Search body/snippet source. |
| `source`      | FTS column | Unindexed   | Record source.              |

Search trust classification treats Teams messages, Outlook messages, calendar events, note metadata, and Obsidian-sourced records as external untrusted data. Other records are treated as local structured data.
