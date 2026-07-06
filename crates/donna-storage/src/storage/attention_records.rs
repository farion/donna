use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::repositories::todo_from_row;
use crate::storage::types::{AttentionItem, NewAttentionItem, StoredTodo};
use rusqlite::{OptionalExtension, Row, params};

impl LocalStore {
    pub fn create_todo_reminder_attention(
        &self,
        at: i64,
    ) -> Result<Option<AttentionItem>, StorageError> {
        let Some(todo) = self.todo_for_reminder(at)? else {
            return Ok(None);
        };

        self.create_attention_item(&NewAttentionItem {
            source_type: "todo_reminder".to_owned(),
            source_id: Some(todo.id),
            level: todo_reminder_level(&todo.severity).to_owned(),
            title: "Open todo".to_owned(),
            body: Some(todo.title),
            due_at: Some(at),
            payload: Some(format!(r#"{{"todo_id":{}}}"#, todo.id)),
        })
        .map(Some)
    }

    pub fn todo_for_reminder(&self, at: i64) -> Result<Option<StoredTodo>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, title, notes, status, source, related_topic, severity, due_at,
                    snoozed_until, stale_at, created_at, updated_at, completed_at,
                    dismissed_at
                 FROM todos todo
                 WHERE todo.status = 'open'
                    AND (todo.snoozed_until IS NULL OR todo.snoozed_until <= ?1)
                    AND NOT EXISTS (
                        SELECT 1 FROM attention_items item
                        WHERE item.source_type = 'todo_reminder'
                            AND item.source_id = todo.id
                            AND item.status IN ('open', 'snoozed')
                    )
                 ORDER BY
                    COALESCE(todo.due_at, 9223372036854775807),
                    CASE todo.severity WHEN 'high' THEN 0 WHEN 'middle' THEN 1 ELSE 2 END,
                    todo.updated_at ASC
                 LIMIT 1",
                [at],
                todo_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn forget_task_reminder_snoozes(&self) -> Result<usize, StorageError> {
        let now = now_seconds()?;
        let todos = self.connection.execute(
            "UPDATE todos
             SET snoozed_until = NULL, updated_at = ?1
             WHERE status = 'open' AND snoozed_until IS NOT NULL",
            [now],
        )?;
        let attention_items = self.connection.execute(
            "UPDATE attention_items
             SET status = 'open', snoozed_until = NULL, feedback = NULL, updated_at = ?1
             WHERE source_type = 'todo_reminder' AND status = 'snoozed'",
            [now],
        )?;

        Ok(todos + attention_items)
    }

    pub fn create_attention_item(
        &self,
        input: &NewAttentionItem,
    ) -> Result<AttentionItem, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO attention_items (
                source_type, source_id, level, title, body, due_at, payload,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &input.source_type,
                input.source_id,
                &input.level,
                &input.title,
                &input.body,
                input.due_at,
                &input.payload,
                now,
                now
            ],
        )?;

        self.attention_item(self.connection.last_insert_rowid())
    }

    pub fn attention_item(&self, id: i64) -> Result<AttentionItem, StorageError> {
        self.connection
            .query_row(
                "SELECT id, source_type, source_id, level, title, body, status,
                    due_at, snoozed_until, dismissed_at, completed_at, feedback,
                    payload, created_at, updated_at
                 FROM attention_items
                 WHERE id = ?1",
                [id],
                attention_item_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn ready_attention_items(&self, at: i64) -> Result<Vec<AttentionItem>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, source_type, source_id, level, title, body, status,
                due_at, snoozed_until, dismissed_at, completed_at, feedback,
                payload, created_at, updated_at
             FROM attention_items
             WHERE status = 'open'
                OR (status = 'snoozed' AND snoozed_until IS NOT NULL AND snoozed_until <= ?1)
             ORDER BY
                CASE level
                    WHEN 'critical' THEN 0
                    WHEN 'important' THEN 1
                    WHEN 'normal' THEN 2
                    ELSE 3
                END,
                coalesce(due_at, created_at)",
        )?;
        let items = statement
            .query_map([at], attention_item_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub fn complete_attention_item(&self, id: i64) -> Result<AttentionItem, StorageError> {
        self.update_attention_item(id, "done", None, None)
    }

    pub fn dismiss_attention_item(
        &self,
        id: i64,
        feedback: Option<&str>,
    ) -> Result<AttentionItem, StorageError> {
        self.update_attention_item(id, "dismissed", None, feedback)
    }

    pub fn snooze_attention_item(
        &self,
        id: i64,
        snoozed_until: i64,
    ) -> Result<AttentionItem, StorageError> {
        self.update_attention_item(id, "snoozed", Some(snoozed_until), None)
    }

    pub fn record_attention_feedback(
        &self,
        id: i64,
        feedback: &str,
    ) -> Result<AttentionItem, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "UPDATE attention_items
             SET feedback = ?1, updated_at = ?2
             WHERE id = ?3",
            params![feedback, now, id],
        )?;
        self.attention_item(id)
    }

    fn update_attention_item(
        &self,
        id: i64,
        status: &str,
        snoozed_until: Option<i64>,
        feedback: Option<&str>,
    ) -> Result<AttentionItem, StorageError> {
        let now = now_seconds()?;
        let completed_at = (status == "done").then_some(now);
        let dismissed_at = (status == "dismissed").then_some(now);

        self.connection.execute(
            "UPDATE attention_items
             SET status = ?1,
                 snoozed_until = ?2,
                 completed_at = ?3,
                 dismissed_at = ?4,
                 feedback = coalesce(?5, feedback),
                 updated_at = ?6
             WHERE id = ?7",
            params![
                status,
                snoozed_until,
                completed_at,
                dismissed_at,
                feedback,
                now,
                id
            ],
        )?;

        self.attention_item(id)
    }
}

fn todo_reminder_level(severity: &str) -> &'static str {
    match severity {
        "high" => "important",
        "low" => "info",
        _ => "normal",
    }
}

fn attention_item_from_row(row: &Row<'_>) -> rusqlite::Result<AttentionItem> {
    Ok(AttentionItem {
        id: row.get(0)?,
        source_type: row.get(1)?,
        source_id: row.get(2)?,
        level: row.get(3)?,
        title: row.get(4)?,
        body: row.get(5)?,
        status: row.get(6)?,
        due_at: row.get(7)?,
        snoozed_until: row.get(8)?,
        dismissed_at: row.get(9)?,
        completed_at: row.get(10)?,
        feedback: row.get(11)?,
        payload: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NewTodo;

    #[test]
    fn snooze_dismiss_and_feedback_are_persisted() {
        let store = LocalStore::in_memory().expect("store");
        let item = store
            .create_attention_item(&NewAttentionItem {
                source_type: "follow_up".to_owned(),
                source_id: Some(7),
                level: "important".to_owned(),
                title: "Anna is waiting".to_owned(),
                body: Some("Reply about billing".to_owned()),
                due_at: Some(100),
                payload: None,
            })
            .expect("attention");

        let snoozed = store
            .snooze_attention_item(item.id, 500)
            .expect("snooze attention");
        assert_eq!(snoozed.status, "snoozed");
        assert_eq!(snoozed.snoozed_until, Some(500));
        assert!(store.ready_attention_items(499).expect("ready").is_empty());
        assert_eq!(store.ready_attention_items(500).expect("ready").len(), 1);

        let dismissed = store
            .dismiss_attention_item(item.id, Some("not_important"))
            .expect("dismiss attention");
        assert_eq!(dismissed.status, "dismissed");
        assert_eq!(dismissed.feedback.as_deref(), Some("not_important"));
        assert!(dismissed.dismissed_at.is_some());
        assert!(store.ready_attention_items(600).expect("ready").is_empty());
    }

    #[test]
    fn todo_reminder_selects_due_then_severe_todo() {
        let store = LocalStore::in_memory().expect("store");
        let later = store
            .create_todo(&NewTodo {
                title: "later high".to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: "high".to_owned(),
                due_at: Some(2_000),
            })
            .expect("later");
        let earliest = store
            .create_todo(&NewTodo {
                title: "earliest low".to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: "low".to_owned(),
                due_at: Some(1_000),
            })
            .expect("earliest");

        let item = store
            .create_todo_reminder_attention(900)
            .expect("reminder")
            .expect("item");

        assert_eq!(item.source_type, "todo_reminder");
        assert_eq!(item.source_id, Some(earliest.id));
        assert_eq!(item.level, "info");
        assert_eq!(item.body.as_deref(), Some("earliest low"));

        let duplicate = store
            .create_todo_reminder_attention(900)
            .expect("duplicate check")
            .expect("second item for next todo");
        assert_eq!(duplicate.source_id, Some(later.id));
    }

    #[test]
    fn todo_severity_update_refreshes_open_reminder_level() {
        let store = LocalStore::in_memory().expect("store");
        let todo = store
            .create_todo(&NewTodo {
                title: "fix the thing".to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: "middle".to_owned(),
                due_at: None,
            })
            .expect("todo");
        let item = store
            .create_todo_reminder_attention(900)
            .expect("reminder")
            .expect("item");
        assert_eq!(item.level, "normal");

        store
            .update_todo_severity(todo.id, "high")
            .expect("update severity");

        assert_eq!(
            store.attention_item(item.id).expect("updated item").level,
            "important"
        );
    }

    #[test]
    fn todo_reminder_respects_todo_snooze_until() {
        let store = LocalStore::in_memory().expect("store");
        let todo = store
            .create_todo(&NewTodo {
                title: "snoozed".to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: "high".to_owned(),
                due_at: None,
            })
            .expect("todo");
        store.snooze_todo_until(todo.id, 2_000).expect("snooze");

        assert!(
            store
                .create_todo_reminder_attention(1_999)
                .expect("early")
                .is_none()
        );
        assert!(
            store
                .create_todo_reminder_attention(2_000)
                .expect("ready")
                .is_some()
        );
    }

    #[test]
    fn forget_task_reminder_snoozes_reopens_task_snoozes() {
        let store = LocalStore::in_memory().expect("store");
        let todo = store
            .create_todo(&NewTodo {
                title: "ignored".to_owned(),
                notes: None,
                source: "test".to_owned(),
                related_topic: None,
                severity: "middle".to_owned(),
                due_at: None,
            })
            .expect("todo");
        store
            .snooze_todo_until(todo.id, 2_000)
            .expect("snooze todo");
        let item = store
            .create_attention_item(&NewAttentionItem {
                source_type: "todo_reminder".to_owned(),
                source_id: Some(todo.id),
                level: "normal".to_owned(),
                title: "Open todo".to_owned(),
                body: Some("ignored".to_owned()),
                due_at: Some(1_000),
                payload: None,
            })
            .expect("attention");
        store
            .snooze_attention_item(item.id, 2_000)
            .expect("snooze attention");

        let changed = store
            .forget_task_reminder_snoozes()
            .expect("forget snoozes");

        assert_eq!(changed, 2);
        assert_eq!(store.todo(todo.id).expect("todo").snoozed_until, None);
        let attention = store.attention_item(item.id).expect("attention");
        assert_eq!(attention.status, "open");
        assert_eq!(attention.snoozed_until, None);
    }
}
