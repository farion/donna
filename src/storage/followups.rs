use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::repositories::follow_up_from_row;
use crate::storage::types::FollowUp;
use rusqlite::params;

impl LocalStore {
    pub fn update_follow_up_status(
        &self,
        id: i64,
        status: &str,
        snoozed_until: Option<i64>,
    ) -> Result<FollowUp, StorageError> {
        let now = now_seconds()?;
        let resolved_at = (status == "done").then_some(now);
        let dismissed_at = (status == "dismissed").then_some(now);
        let stale_at = (status == "stale").then_some(now);

        self.connection.execute(
            "UPDATE follow_ups
             SET status = ?1,
                 snoozed_until = ?2,
                 resolved_at = ?3,
                 dismissed_at = ?4,
                 stale_at = coalesce(?5, stale_at),
                 updated_at = ?6
             WHERE id = ?7",
            params![
                status,
                snoozed_until,
                resolved_at,
                dismissed_at,
                stale_at,
                now,
                id
            ],
        )?;

        self.follow_up(id)
    }

    pub fn stale_follow_ups(&self, at: i64) -> Result<Vec<FollowUp>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, direction, person_id, status, source, summary, due_at,
                stale_at, snoozed_until, created_at, updated_at, resolved_at,
                dismissed_at
             FROM follow_ups
             WHERE status IN ('open', 'stale')
                AND (
                    (due_at IS NOT NULL AND due_at <= ?1)
                    OR (stale_at IS NOT NULL AND stale_at <= ?1)
                )
             ORDER BY coalesce(due_at, stale_at), created_at",
        )?;
        let follow_ups = statement
            .query_map([at], follow_up_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(follow_ups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NewFollowUp;

    #[test]
    fn follow_up_state_transitions_and_stale_query_work() {
        let store = LocalStore::in_memory().expect("store");
        let follow_up = store
            .create_follow_up(&NewFollowUp {
                direction: "waiting_for_them".to_owned(),
                person_id: None,
                source: "outlook".to_owned(),
                summary: "Waiting for contract answer".to_owned(),
                due_at: Some(100),
            })
            .expect("follow-up");

        assert_eq!(store.stale_follow_ups(99).expect("not due").len(), 0);
        assert_eq!(store.stale_follow_ups(100).expect("due").len(), 1);

        let snoozed = store
            .update_follow_up_status(follow_up.id, "snoozed", Some(500))
            .expect("snooze");
        assert_eq!(snoozed.status, "snoozed");
        assert_eq!(snoozed.snoozed_until, Some(500));
        assert_eq!(store.stale_follow_ups(600).expect("snoozed").len(), 0);

        let done = store
            .update_follow_up_status(follow_up.id, "done", None)
            .expect("done");
        assert_eq!(done.status, "done");
        assert!(done.resolved_at.is_some());
    }
}
