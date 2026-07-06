use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use rusqlite::{Row, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuditEntry {
    pub action_type: String,
    pub target_system: String,
    pub summary: String,
    pub approval_at: i64,
    pub execution_at: i64,
    pub result: String,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEntry {
    pub id: i64,
    pub action_type: String,
    pub target_system: String,
    pub summary: String,
    pub approval_at: i64,
    pub execution_at: i64,
    pub result: String,
    pub external_id: Option<String>,
    pub created_at: i64,
}

impl LocalStore {
    pub fn record_audit_entry(&self, entry: &NewAuditEntry) -> Result<AuditEntry, StorageError> {
        let created_at = now_seconds()?;
        self.connection.execute(
            "INSERT INTO audit_log (
                action_type, target_system, summary, approval_at, execution_at,
                result, external_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &entry.action_type,
                &entry.target_system,
                &entry.summary,
                entry.approval_at,
                entry.execution_at,
                &entry.result,
                &entry.external_id,
                created_at
            ],
        )?;

        self.audit_entry(self.connection.last_insert_rowid())
    }

    pub fn audit_entry(&self, id: i64) -> Result<AuditEntry, StorageError> {
        self.connection
            .query_row(
                "SELECT id, action_type, target_system, summary, approval_at,
                    execution_at, result, external_id, created_at
                 FROM audit_log
                 WHERE id = ?1",
                [id],
                audit_from_row,
            )
            .map_err(StorageError::from)
    }
}

fn audit_from_row(row: &Row<'_>) -> rusqlite::Result<AuditEntry> {
    Ok(AuditEntry {
        id: row.get(0)?,
        action_type: row.get(1)?,
        target_system: row.get(2)?,
        summary: row.get(3)?,
        approval_at: row.get(4)?,
        execution_at: row.get(5)?,
        result: row.get(6)?,
        external_id: row.get(7)?,
        created_at: row.get(8)?,
    })
}

#[cfg(test)]
mod tests {
    use super::NewAuditEntry;
    use crate::storage::LocalStore;

    #[test]
    fn writes_and_reads_audit_entry() {
        let store = LocalStore::in_memory().expect("store");
        let entry = NewAuditEntry {
            action_type: "send_mail".to_owned(),
            target_system: "outlook".to_owned(),
            summary: "Sent approved reply to billing thread".to_owned(),
            approval_at: 10,
            execution_at: 12,
            result: "sent".to_owned(),
            external_id: Some("message-1".to_owned()),
        };

        let stored = store.record_audit_entry(&entry).expect("write audit");
        let loaded = store.audit_entry(stored.id).expect("read audit");

        assert_eq!(loaded.action_type, "send_mail");
        assert_eq!(loaded.target_system, "outlook");
        assert_eq!(loaded.external_id.as_deref(), Some("message-1"));
    }
}
