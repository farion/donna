use crate::approval::{ApprovalDecision, ApprovalRequest, ExternalActionKind};
use crate::microsoft::error::GraphError;
use crate::microsoft::sync::run_sync;
use crate::microsoft::types::{ActionReceipt, CalendarEventDraft, GraphSyncPage, SyncReport};
use crate::storage::{CalendarEvent, LocalStore, NewCalendarEvent, NewTaskFinding, TaskFinding};
use std::time::{SystemTime, UNIX_EPOCH};

pub const CALENDAR_SOURCE: &str = "calendar";

pub trait CalendarGraphClient {
    fn sync_events(
        &self,
        delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewCalendarEvent>, GraphError>;

    fn create_event(&self, draft: &CalendarEventDraft) -> Result<ActionReceipt, GraphError>;
    fn update_event(&self, draft: &CalendarEventDraft) -> Result<ActionReceipt, GraphError>;
    fn delete_event(&self, external_id: &str) -> Result<ActionReceipt, GraphError>;
}

#[derive(Debug, Clone)]
pub struct CalendarAdapter<C> {
    client: C,
}

impl<C> CalendarAdapter<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn prepare_create_event(&self, draft: &CalendarEventDraft) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::CreateCalendarEvent,
            CALENDAR_SOURCE,
            draft.create_summary(),
        )
    }

    pub fn prepare_update_event(&self, draft: &CalendarEventDraft) -> ApprovalRequest {
        let request = ApprovalRequest::new(
            ExternalActionKind::UpdateCalendarEvent,
            CALENDAR_SOURCE,
            draft.update_summary(),
        );

        match &draft.external_id {
            Some(external_id) => request.with_external_id(external_id),
            None => request,
        }
    }

    pub fn prepare_delete_event(&self, external_id: &str) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::DeleteCalendarEvent,
            CALENDAR_SOURCE,
            format!("Delete calendar event {external_id}"),
        )
        .with_external_id(external_id)
    }
}

impl<C> CalendarAdapter<C>
where
    C: CalendarGraphClient,
{
    pub fn sync_events(&self, store: &LocalStore) -> Result<SyncReport, GraphError> {
        run_sync(
            store,
            CALENDAR_SOURCE,
            |delta_link| self.client.sync_events(delta_link),
            |event| store.upsert_calendar_event(event).map(|_| ()),
        )
    }

    pub fn create_event(
        &self,
        store: &LocalStore,
        draft: &CalendarEventDraft,
        decision: ApprovalDecision,
    ) -> Result<ActionReceipt, GraphError> {
        ensure_online(store)?;
        let approved = self.prepare_create_event(draft).approve(decision)?;
        let receipt = self.client.create_event(draft)?;
        record_action(store, approved, receipt)
    }

    pub fn update_event(
        &self,
        store: &LocalStore,
        draft: &CalendarEventDraft,
        decision: ApprovalDecision,
    ) -> Result<ActionReceipt, GraphError> {
        ensure_online(store)?;
        let approved = self.prepare_update_event(draft).approve(decision)?;
        let receipt = self.client.update_event(draft)?;
        record_action(store, approved, receipt)
    }

    pub fn delete_event(
        &self,
        store: &LocalStore,
        external_id: &str,
        decision: ApprovalDecision,
    ) -> Result<ActionReceipt, GraphError> {
        ensure_online(store)?;
        let approved = self.prepare_delete_event(external_id).approve(decision)?;
        let receipt = self.client.delete_event(external_id)?;
        record_action(store, approved, receipt)
    }
}

impl<C> CalendarAdapter<C> {
    pub fn collisions(
        &self,
        store: &LocalStore,
        starts_at: i64,
        ends_at: i64,
    ) -> Result<Vec<CalendarEvent>, GraphError> {
        store
            .calendar_collisions(starts_at, ends_at)
            .map_err(GraphError::from)
    }

    pub fn record_collision_findings(
        &self,
        store: &LocalStore,
        proposed_summary: &str,
        starts_at: i64,
        ends_at: i64,
    ) -> Result<Vec<TaskFinding>, GraphError> {
        let collisions = self.collisions(store, starts_at, ends_at)?;
        let mut findings = Vec::with_capacity(collisions.len());

        for event in collisions {
            let payload = serde_json::json!({
                "proposed": proposed_summary,
                "starts_at": starts_at,
                "ends_at": ends_at,
                "conflicting_event_id": event.external_id,
                "conflicting_subject": event.subject,
                "original_timezone": event.original_timezone,
            });

            findings.push(store.create_task_finding(&NewTaskFinding {
                task_run_id: None,
                kind: "calendar_collision".to_owned(),
                summary: format!(
                    "Calendar collision for {proposed_summary}: {}",
                    event.subject.unwrap_or_else(|| event.external_id.clone())
                ),
                source: CALENDAR_SOURCE.to_owned(),
                payload: Some(payload.to_string()),
            })?);
        }

        Ok(findings)
    }
}

fn record_action(
    store: &LocalStore,
    approved: crate::approval::ApprovedAction,
    receipt: ActionReceipt,
) -> Result<ActionReceipt, GraphError> {
    let external_id = receipt.external_id.clone();
    let mut audit = approved.audit_entry(now_seconds()?, receipt.result.clone());
    audit.external_id = external_id;
    store.record_audit_entry(&audit)?;
    Ok(receipt)
}

fn ensure_online(store: &LocalStore) -> Result<(), GraphError> {
    if store.is_offline()? {
        return Err(GraphError::Offline);
    }
    Ok(())
}

fn now_seconds() -> Result<i64, GraphError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(elapsed.as_secs() as i64)
}
