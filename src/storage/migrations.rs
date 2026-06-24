use rusqlite::{Connection, params};

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

pub(super) fn apply_migrations(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at INTEGER NOT NULL
        );",
    )?;

    for migration in migrations() {
        let already_applied: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;

        if already_applied {
            continue;
        }

        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name, applied_at)
             VALUES (?1, ?2, CAST(strftime('%s', 'now') AS INTEGER))",
            params![migration.version, migration.name],
        )?;
        transaction.commit()?;
    }

    Ok(())
}

fn migrations() -> &'static [Migration] {
    &[
        Migration {
            version: 1,
            name: "local_foundation",
            sql: LOCAL_FOUNDATION,
        },
        Migration {
            version: 2,
            name: "attention_workflows",
            sql: ATTENTION_WORKFLOWS,
        },
    ]
}

const LOCAL_FOUNDATION: &str = r#"
CREATE TABLE memories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    memory_type TEXT NOT NULL,
    content TEXT NOT NULL,
    source TEXT NOT NULL,
    confidence REAL NOT NULL DEFAULT 1.0,
    importance INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    expires_at INTEGER,
    forgotten_at INTEGER
);

CREATE TABLE todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'done', 'dismissed', 'snoozed', 'stale')),
    source TEXT NOT NULL,
    related_topic TEXT,
    due_at INTEGER,
    snoozed_until INTEGER,
    stale_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,
    dismissed_at INTEGER
);

CREATE TABLE people (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    display_name TEXT NOT NULL,
    context TEXT,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE person_aliases (
    person_id INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    alias TEXT NOT NULL,
    PRIMARY KEY (person_id, alias)
);

CREATE TABLE person_emails (
    person_id INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    PRIMARY KEY (person_id, email)
);

CREATE TABLE person_teams_ids (
    person_id INTEGER NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    teams_id TEXT NOT NULL,
    PRIMARY KEY (person_id, teams_id)
);

CREATE TABLE follow_ups (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    direction TEXT NOT NULL CHECK (direction IN ('waiting_for_me', 'waiting_for_them')),
    person_id INTEGER REFERENCES people(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'done', 'dismissed', 'snoozed', 'stale')),
    source TEXT NOT NULL,
    summary TEXT NOT NULL,
    due_at INTEGER,
    stale_at INTEGER,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    resolved_at INTEGER
);

CREATE TABLE task_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL,
    task_model_id TEXT NOT NULL,
    status TEXT NOT NULL,
    prompt_path TEXT,
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    error_summary TEXT
);

CREATE TABLE task_findings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_run_id INTEGER REFERENCES task_runs(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    summary TEXT NOT NULL,
    source TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    dismissed_at INTEGER,
    payload TEXT
);

CREATE TABLE teams_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id TEXT NOT NULL UNIQUE,
    chat_id TEXT NOT NULL,
    sender_name TEXT,
    sender_external_id TEXT,
    body TEXT NOT NULL,
    importance TEXT,
    web_url TEXT,
    sent_at INTEGER,
    synced_at INTEGER NOT NULL,
    etag TEXT,
    change_key TEXT,
    is_deleted INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0, 1))
);

CREATE TABLE outlook_messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id TEXT NOT NULL UNIQUE,
    folder_id TEXT,
    subject TEXT,
    sender_name TEXT,
    sender_email TEXT,
    body_preview TEXT,
    received_at INTEGER,
    synced_at INTEGER NOT NULL,
    etag TEXT,
    change_key TEXT,
    is_deleted INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0, 1))
);

CREATE TABLE calendar_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id TEXT NOT NULL UNIQUE,
    subject TEXT,
    organizer_name TEXT,
    organizer_email TEXT,
    starts_at INTEGER,
    ends_at INTEGER,
    original_timezone TEXT,
    show_as TEXT,
    synced_at INTEGER NOT NULL,
    etag TEXT,
    change_key TEXT,
    is_cancelled INTEGER NOT NULL DEFAULT 0 CHECK (is_cancelled IN (0, 1)),
    is_deleted INTEGER NOT NULL DEFAULT 0 CHECK (is_deleted IN (0, 1))
);

CREATE TABLE notes_metadata (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_path TEXT NOT NULL,
    note_path TEXT NOT NULL,
    title TEXT,
    headings TEXT,
    tags TEXT,
    links TEXT,
    modified_at INTEGER,
    indexed_at INTEGER NOT NULL,
    UNIQUE (vault_path, note_path)
);

CREATE TABLE sync_state (
    source TEXT PRIMARY KEY,
    cursor TEXT,
    delta_link TEXT,
    last_sync_at INTEGER,
    last_error TEXT,
    is_stale INTEGER NOT NULL DEFAULT 0 CHECK (is_stale IN (0, 1)),
    updated_at INTEGER NOT NULL
);

CREATE TABLE local_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE audit_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action_type TEXT NOT NULL,
    target_system TEXT NOT NULL,
    summary TEXT NOT NULL,
    approval_at INTEGER NOT NULL,
    execution_at INTEGER NOT NULL,
    result TEXT NOT NULL,
    external_id TEXT,
    created_at INTEGER NOT NULL
);

CREATE VIRTUAL TABLE search_index USING fts5(
    record_type,
    record_id UNINDEXED,
    title,
    body,
    source UNINDEXED
);
"#;

const ATTENTION_WORKFLOWS: &str = r#"
ALTER TABLE follow_ups ADD COLUMN snoozed_until INTEGER;
ALTER TABLE follow_ups ADD COLUMN dismissed_at INTEGER;

CREATE TABLE attention_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type TEXT NOT NULL,
    source_id INTEGER,
    level TEXT NOT NULL
        CHECK (level IN ('info', 'normal', 'important', 'critical')),
    title TEXT NOT NULL,
    body TEXT,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'done', 'dismissed', 'snoozed', 'stale')),
    due_at INTEGER,
    snoozed_until INTEGER,
    dismissed_at INTEGER,
    completed_at INTEGER,
    feedback TEXT,
    payload TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
"#;

#[cfg(test)]
mod tests {
    use super::apply_migrations;
    use rusqlite::Connection;

    #[test]
    fn applies_migrations_to_fresh_database() {
        let connection = Connection::open_in_memory().expect("open database");

        apply_migrations(&connection).expect("apply migrations");

        let version_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count migrations");
        assert_eq!(version_count, 2);

        for table in [
            "memories",
            "todos",
            "people",
            "follow_ups",
            "teams_messages",
            "outlook_messages",
            "calendar_events",
            "task_runs",
            "task_findings",
            "sync_state",
            "audit_log",
            "attention_items",
            "search_index",
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE name = ?1)",
                    [table],
                    |row| row.get(0),
                )
                .expect("table exists query");
            assert!(exists, "{table} should exist");
        }
    }

    #[test]
    fn schema_does_not_create_raw_donna_chat_tables() {
        let connection = Connection::open_in_memory().expect("open database");
        apply_migrations(&connection).expect("apply migrations");

        let mut statement = connection
            .prepare(
                "SELECT name FROM sqlite_schema
                 WHERE type IN ('table', 'view')
                 ORDER BY name",
            )
            .expect("prepare schema query");
        let names = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query schema")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect names");

        assert!(!names.iter().any(|name| name == "chat_messages"));
        assert!(!names.iter().any(|name| name == "chat_transcripts"));
        assert!(!names.iter().any(|name| name == "donna_chat"));
    }
}
