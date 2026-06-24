use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::types::{
    DataFreshness, FollowUp, NewFollowUp, NewMemory, NewPerson, NewSyncState, NewTodo, Person,
    StoredMemory, StoredTodo, SyncState,
};
use rusqlite::{OptionalExtension, Row, params};

impl LocalStore {
    pub fn create_memory(&self, input: &NewMemory) -> Result<StoredMemory, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO memories (
                memory_type, content, source, confidence, importance,
                created_at, updated_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &input.memory_type,
                &input.content,
                &input.source,
                input.confidence,
                input.importance,
                now,
                now,
                input.expires_at
            ],
        )?;

        let memory = self.memory(self.connection.last_insert_rowid())?;
        self.insert_search_record(
            "memory",
            memory.id,
            &memory.memory_type,
            &memory.content,
            &memory.source,
        )?;
        Ok(memory)
    }

    pub fn memory(&self, id: i64) -> Result<StoredMemory, StorageError> {
        self.connection
            .query_row(
                "SELECT id, memory_type, content, source, confidence, importance,
                    created_at, updated_at, expires_at, forgotten_at
                 FROM memories
                 WHERE id = ?1",
                [id],
                memory_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn forget_memory(&self, id: i64) -> Result<StoredMemory, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "UPDATE memories
             SET forgotten_at = ?1, updated_at = ?1
             WHERE id = ?2",
            params![now, id],
        )?;

        self.memory(id)
    }

    pub fn create_todo(&self, input: &NewTodo) -> Result<StoredTodo, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO todos (
                title, notes, source, related_topic, due_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &input.title,
                &input.notes,
                &input.source,
                &input.related_topic,
                input.due_at,
                now,
                now
            ],
        )?;

        let todo = self.todo(self.connection.last_insert_rowid())?;
        self.insert_search_record(
            "todo",
            todo.id,
            &todo.title,
            todo.notes.as_deref().unwrap_or(""),
            &todo.source,
        )?;
        Ok(todo)
    }

    pub fn todo(&self, id: i64) -> Result<StoredTodo, StorageError> {
        self.connection
            .query_row(
                "SELECT id, title, notes, status, source, related_topic, due_at,
                    snoozed_until, stale_at, created_at, updated_at, completed_at,
                    dismissed_at
                 FROM todos
                 WHERE id = ?1",
                [id],
                todo_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn update_todo_status(&self, id: i64, status: &str) -> Result<StoredTodo, StorageError> {
        let now = now_seconds()?;
        let completed_at = (status == "done").then_some(now);
        let dismissed_at = (status == "dismissed").then_some(now);
        let stale_at = (status == "stale").then_some(now);

        self.connection.execute(
            "UPDATE todos
             SET status = ?1, updated_at = ?2, completed_at = ?3,
                 dismissed_at = ?4, stale_at = ?5
             WHERE id = ?6",
            params![status, now, completed_at, dismissed_at, stale_at, id],
        )?;

        self.todo(id)
    }

    pub fn create_person(&self, input: &NewPerson) -> Result<Person, StorageError> {
        let now = now_seconds()?;
        let transaction = self.connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO people (display_name, context, source, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![&input.display_name, &input.context, &input.source, now, now],
        )?;
        let person_id = transaction.last_insert_rowid();

        for alias in &input.aliases {
            transaction.execute(
                "INSERT OR IGNORE INTO person_aliases (person_id, alias) VALUES (?1, ?2)",
                params![person_id, alias],
            )?;
        }
        for email in &input.emails {
            transaction.execute(
                "INSERT OR IGNORE INTO person_emails (person_id, email) VALUES (?1, ?2)",
                params![person_id, email],
            )?;
        }
        for teams_id in &input.teams_ids {
            transaction.execute(
                "INSERT OR IGNORE INTO person_teams_ids (person_id, teams_id) VALUES (?1, ?2)",
                params![person_id, teams_id],
            )?;
        }

        transaction.commit()?;
        let person = self.person(person_id)?;
        self.insert_search_record(
            "person",
            person.id,
            &person.display_name,
            person.context.as_deref().unwrap_or(""),
            &person.source,
        )?;
        Ok(person)
    }

    pub fn person(&self, id: i64) -> Result<Person, StorageError> {
        let mut person = self
            .connection
            .query_row(
                "SELECT id, display_name, context, source, created_at, updated_at
                 FROM people
                 WHERE id = ?1",
                [id],
                person_from_row,
            )
            .map_err(StorageError::from)?;

        person.aliases = self.collect_person_values(id, "person_aliases", "alias")?;
        person.emails = self.collect_person_values(id, "person_emails", "email")?;
        person.teams_ids = self.collect_person_values(id, "person_teams_ids", "teams_id")?;
        Ok(person)
    }

    pub fn create_follow_up(&self, input: &NewFollowUp) -> Result<FollowUp, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO follow_ups (
                direction, person_id, source, summary, due_at, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &input.direction,
                input.person_id,
                &input.source,
                &input.summary,
                input.due_at,
                now,
                now
            ],
        )?;

        let follow_up = self.follow_up(self.connection.last_insert_rowid())?;
        self.insert_search_record(
            "follow_up",
            follow_up.id,
            &follow_up.direction,
            &follow_up.summary,
            &follow_up.source,
        )?;
        Ok(follow_up)
    }

    pub fn follow_up(&self, id: i64) -> Result<FollowUp, StorageError> {
        self.connection
            .query_row(
                "SELECT id, direction, person_id, status, source, summary, due_at,
                    stale_at, snoozed_until, created_at, updated_at, resolved_at,
                    dismissed_at
                 FROM follow_ups
                 WHERE id = ?1",
                [id],
                follow_up_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn upsert_sync_state(&self, input: &NewSyncState) -> Result<SyncState, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO sync_state (
                source, cursor, delta_link, last_sync_at, last_error, is_stale, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(source) DO UPDATE SET
                cursor = excluded.cursor,
                delta_link = excluded.delta_link,
                last_sync_at = excluded.last_sync_at,
                last_error = excluded.last_error,
                is_stale = excluded.is_stale,
                updated_at = excluded.updated_at",
            params![
                &input.source,
                &input.cursor,
                &input.delta_link,
                input.last_sync_at,
                &input.last_error,
                input.is_stale as i64,
                now
            ],
        )?;

        self.sync_state(&input.source)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows.into())
    }

    pub fn sync_state(&self, source: &str) -> Result<Option<SyncState>, StorageError> {
        self.connection
            .query_row(
                "SELECT source, cursor, delta_link, last_sync_at, last_error,
                    is_stale, updated_at
                 FROM sync_state
                 WHERE source = ?1",
                [source],
                sync_state_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn data_freshness(&self, source: &str) -> Result<DataFreshness, StorageError> {
        match self.sync_state(source)? {
            Some(state) if state.is_stale => Ok(DataFreshness::Stale {
                error: state.last_error,
            }),
            Some(_) => Ok(DataFreshness::Fresh),
            None => Ok(DataFreshness::NeverSynced),
        }
    }

    pub fn set_offline_mode(&self, offline: bool) -> Result<(), StorageError> {
        self.set_local_state("network.offline", if offline { "true" } else { "false" })
    }

    pub fn is_offline(&self) -> Result<bool, StorageError> {
        Ok(self
            .local_state("network.offline")?
            .is_some_and(|value| value == "true"))
    }

    fn set_local_state(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO local_state (key, value, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET
                value = excluded.value,
                updated_at = excluded.updated_at",
            params![key, value, now],
        )?;
        Ok(())
    }

    fn local_state(&self, key: &str) -> Result<Option<String>, StorageError> {
        self.connection
            .query_row(
                "SELECT value FROM local_state WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()
            .map_err(StorageError::from)
    }

    fn collect_person_values(
        &self,
        person_id: i64,
        table: &str,
        column: &str,
    ) -> Result<Vec<String>, StorageError> {
        let sql = format!("SELECT {column} FROM {table} WHERE person_id = ?1 ORDER BY {column}");
        let mut statement = self.connection.prepare(&sql)?;
        let values = statement
            .query_map([person_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(values)
    }

    pub(super) fn replace_search_record(
        &self,
        record_type: &str,
        record_id: i64,
        title: &str,
        body: &str,
        source: &str,
    ) -> Result<(), StorageError> {
        self.delete_search_record(record_type, record_id)?;
        self.insert_search_record(record_type, record_id, title, body, source)
    }

    pub(super) fn delete_search_record(
        &self,
        record_type: &str,
        record_id: i64,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "DELETE FROM search_index WHERE record_type = ?1 AND record_id = ?2",
            params![record_type, record_id],
        )?;
        Ok(())
    }

    pub(super) fn insert_search_record(
        &self,
        record_type: &str,
        record_id: i64,
        title: &str,
        body: &str,
        source: &str,
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "INSERT INTO search_index (record_type, record_id, title, body, source)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![record_type, record_id, title, body, source],
        )?;
        Ok(())
    }
}

fn memory_from_row(row: &Row<'_>) -> rusqlite::Result<StoredMemory> {
    Ok(StoredMemory {
        id: row.get(0)?,
        memory_type: row.get(1)?,
        content: row.get(2)?,
        source: row.get(3)?,
        confidence: row.get(4)?,
        importance: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
        expires_at: row.get(8)?,
        forgotten_at: row.get(9)?,
    })
}

fn todo_from_row(row: &Row<'_>) -> rusqlite::Result<StoredTodo> {
    Ok(StoredTodo {
        id: row.get(0)?,
        title: row.get(1)?,
        notes: row.get(2)?,
        status: row.get(3)?,
        source: row.get(4)?,
        related_topic: row.get(5)?,
        due_at: row.get(6)?,
        snoozed_until: row.get(7)?,
        stale_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        completed_at: row.get(11)?,
        dismissed_at: row.get(12)?,
    })
}

fn person_from_row(row: &Row<'_>) -> rusqlite::Result<Person> {
    Ok(Person {
        id: row.get(0)?,
        display_name: row.get(1)?,
        aliases: Vec::new(),
        emails: Vec::new(),
        teams_ids: Vec::new(),
        context: row.get(2)?,
        source: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

pub(super) fn follow_up_from_row(row: &Row<'_>) -> rusqlite::Result<FollowUp> {
    Ok(FollowUp {
        id: row.get(0)?,
        direction: row.get(1)?,
        person_id: row.get(2)?,
        status: row.get(3)?,
        source: row.get(4)?,
        summary: row.get(5)?,
        due_at: row.get(6)?,
        stale_at: row.get(7)?,
        snoozed_until: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        resolved_at: row.get(11)?,
        dismissed_at: row.get(12)?,
    })
}

fn sync_state_from_row(row: &Row<'_>) -> rusqlite::Result<SyncState> {
    let is_stale: i64 = row.get(5)?;
    Ok(SyncState {
        source: row.get(0)?,
        cursor: row.get(1)?,
        delta_link: row.get(2)?,
        last_sync_at: row.get(3)?,
        last_error: row.get(4)?,
        is_stale: is_stale != 0,
        updated_at: row.get(6)?,
    })
}
