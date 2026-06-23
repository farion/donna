use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::types::{NewTaskFinding, NewTaskRun, TaskFinding, TaskRun};
use rusqlite::{Row, params};

impl LocalStore {
    pub fn start_task_run(&self, input: &NewTaskRun) -> Result<TaskRun, StorageError> {
        let started_at = now_seconds()?;
        self.connection.execute(
            "INSERT INTO task_runs (
                task_id, task_model_id, status, prompt_path, started_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &input.task_id,
                &input.task_model_id,
                &input.status,
                &input.prompt_path,
                started_at
            ],
        )?;

        self.task_run(self.connection.last_insert_rowid())
    }

    pub fn finish_task_run(
        &self,
        id: i64,
        status: &str,
        error_summary: Option<&str>,
    ) -> Result<TaskRun, StorageError> {
        let finished_at = now_seconds()?;
        self.connection.execute(
            "UPDATE task_runs
             SET status = ?1, finished_at = ?2, error_summary = ?3
             WHERE id = ?4",
            params![status, finished_at, error_summary, id],
        )?;

        self.task_run(id)
    }

    pub fn task_run(&self, id: i64) -> Result<TaskRun, StorageError> {
        self.connection
            .query_row(
                "SELECT id, task_id, task_model_id, status, prompt_path,
                    started_at, finished_at, error_summary
                 FROM task_runs
                 WHERE id = ?1",
                [id],
                task_run_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn create_task_finding(&self, input: &NewTaskFinding) -> Result<TaskFinding, StorageError> {
        let created_at = now_seconds()?;
        self.connection.execute(
            "INSERT INTO task_findings (
                task_run_id, kind, summary, source, created_at, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                input.task_run_id,
                &input.kind,
                &input.summary,
                &input.source,
                created_at,
                &input.payload
            ],
        )?;

        self.task_finding(self.connection.last_insert_rowid())
    }

    pub fn task_finding(&self, id: i64) -> Result<TaskFinding, StorageError> {
        self.connection
            .query_row(
                "SELECT id, task_run_id, kind, summary, source, created_at,
                    dismissed_at, payload
                 FROM task_findings
                 WHERE id = ?1",
                [id],
                task_finding_from_row,
            )
            .map_err(StorageError::from)
    }
}

fn task_run_from_row(row: &Row<'_>) -> rusqlite::Result<TaskRun> {
    Ok(TaskRun {
        id: row.get(0)?,
        task_id: row.get(1)?,
        task_model_id: row.get(2)?,
        status: row.get(3)?,
        prompt_path: row.get(4)?,
        started_at: row.get(5)?,
        finished_at: row.get(6)?,
        error_summary: row.get(7)?,
    })
}

fn task_finding_from_row(row: &Row<'_>) -> rusqlite::Result<TaskFinding> {
    Ok(TaskFinding {
        id: row.get(0)?,
        task_run_id: row.get(1)?,
        kind: row.get(2)?,
        summary: row.get(3)?,
        source: row.get(4)?,
        created_at: row.get(5)?,
        dismissed_at: row.get(6)?,
        payload: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{NewTaskFinding, NewTaskRun};
    use crate::storage::LocalStore;

    #[test]
    fn records_task_run_lifecycle_and_findings() {
        let store = LocalStore::in_memory().expect("store");
        let run = store
            .start_task_run(&NewTaskRun {
                task_id: "daily-planning".to_owned(),
                task_model_id: "ollama-local".to_owned(),
                status: "running".to_owned(),
                prompt_path: Some("daily.md".to_owned()),
            })
            .expect("start run");

        let finding = store
            .create_task_finding(&NewTaskFinding {
                task_run_id: Some(run.id),
                kind: "todo".to_owned(),
                summary: "Review overdue work".to_owned(),
                source: "task:daily-planning".to_owned(),
                payload: None,
            })
            .expect("finding");
        let finished = store
            .finish_task_run(run.id, "done", None)
            .expect("finish run");

        assert_eq!(finished.status, "done");
        assert!(finished.finished_at.is_some());
        assert_eq!(finding.task_run_id, Some(run.id));
    }
}
