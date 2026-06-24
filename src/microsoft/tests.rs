use crate::approval::ApprovalDecision;
use crate::microsoft::auth::{MicrosoftTokenSet, load_microsoft_tokens, store_microsoft_tokens};
use crate::microsoft::calendar::{CalendarAdapter, CalendarGraphClient};
use crate::microsoft::error::GraphError;
use crate::microsoft::outlook::{OUTLOOK_MAIL_SOURCE, OutlookAdapter, OutlookGraphClient};
use crate::microsoft::teams::{TeamsChatAdapter, TeamsChatGraphClient};
use crate::microsoft::types::{
    ActionReceipt, CalendarEventDraft, GraphSyncPage, MailDraft, TeamsChatDraft,
};
use crate::secrets::InMemorySecretStore;
use crate::storage::{
    DataFreshness, LocalStore, NewCalendarEvent, NewOutlookMessage, NewTeamsMessage,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[test]
fn microsoft_tokens_round_trip_through_secret_store() {
    let store = InMemorySecretStore::default();
    let tokens = MicrosoftTokenSet {
        access_token: "access-token".to_owned(),
        refresh_token: Some("refresh-token".to_owned()),
        token_type: "Bearer".to_owned(),
        scope: Some("User.Read Mail.Read".to_owned()),
        expires_at: Some(100),
    };

    store_microsoft_tokens(&store, "donna/microsoft", &tokens).expect("store tokens");
    let loaded = load_microsoft_tokens(&store, "donna/microsoft").expect("load tokens");

    assert_eq!(loaded, Some(tokens));
}

#[test]
fn outlook_sync_updates_state_and_persists_message() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockOutlookClient::with_messages(vec![mail_message("message-1")]);
    let adapter = OutlookAdapter::new(client.clone());

    let report = adapter.sync_mail(&store).expect("sync mail");

    assert_eq!(report.synced_records, 1);
    assert_eq!(report.delta_link.as_deref(), Some("delta-2"));
    assert_eq!(
        store
            .outlook_message_by_external_id("message-1")
            .expect("message")
            .sender_email
            .as_deref(),
        Some("anna@example.com")
    );
    assert_eq!(
        store
            .sync_state(OUTLOOK_MAIL_SOURCE)
            .expect("state")
            .expect("present")
            .delta_link
            .as_deref(),
        Some("delta-2")
    );
    assert_eq!(client.last_delta.borrow().as_deref(), None);
}

#[test]
fn graph_sync_failure_marks_source_stale_with_clear_error() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockOutlookClient::failing_admin_consent();
    let adapter = OutlookAdapter::new(client);

    let error = adapter.sync_mail(&store).expect_err("sync fails");

    assert!(error.is_permission_problem());
    assert_eq!(
        store
            .data_freshness(OUTLOOK_MAIL_SOURCE)
            .expect("freshness"),
        DataFreshness::Stale {
            error: Some(
                "Microsoft Graph needs tenant admin consent before Donna can continue: AADSTS65001"
                    .to_owned()
            )
        }
    );
}

#[test]
fn offline_sync_marks_stale_without_calling_graph() {
    let store = LocalStore::in_memory().expect("store");
    store.set_offline_mode(true).expect("offline");
    let client = MockOutlookClient::with_messages(vec![mail_message("message-1")]);
    let adapter = OutlookAdapter::new(client.clone());

    let error = adapter.sync_mail(&store).expect_err("offline");

    assert!(matches!(error, GraphError::Offline));
    assert_eq!(client.sync_calls.get(), 0);
    assert_eq!(
        store
            .data_freshness(OUTLOOK_MAIL_SOURCE)
            .expect("freshness"),
        DataFreshness::Stale {
            error: Some("offline".to_owned())
        }
    );
}

#[test]
fn pending_approval_does_not_send_outlook_mail() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockOutlookClient::with_messages(Vec::new());
    let adapter = OutlookAdapter::new(client.clone());
    let draft = MailDraft {
        to: vec!["anna@example.com".to_owned()],
        subject: "Question".to_owned(),
        body: "External body".to_owned(),
    };

    let error = adapter
        .send_mail(&store, &draft, ApprovalDecision::Pending)
        .expect_err("approval required");

    assert!(matches!(error, GraphError::Approval(_)));
    assert_eq!(client.send_calls.get(), 0);
}

#[test]
fn approved_teams_send_records_audit_entry() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockTeamsChatClient::default();
    let adapter = TeamsChatAdapter::new(client.clone());
    let draft = TeamsChatDraft {
        chat_id: "chat-1".to_owned(),
        body: "Approved reply".to_owned(),
    };

    let receipt = adapter
        .send_message(
            &store,
            &draft,
            ApprovalDecision::Approved { approved_at: 10 },
        )
        .expect("send");
    let audit = store.audit_entry(1).expect("audit");

    assert_eq!(receipt.external_id.as_deref(), Some("teams-sent-1"));
    assert_eq!(client.send_calls.get(), 1);
    assert_eq!(audit.action_type, "send_teams_message");
    assert_eq!(audit.target_system, "teams.chat");
    assert_eq!(audit.external_id.as_deref(), Some("teams-sent-1"));
}

#[test]
fn calendar_collisions_ignore_free_cancelled_deleted_and_record_findings() {
    let store = LocalStore::in_memory().expect("store");
    store
        .upsert_calendar_event(&calendar_event(
            "busy", "Busy", 100, 200, "busy", false, false,
        ))
        .expect("busy");
    store
        .upsert_calendar_event(&calendar_event(
            "free", "Free", 120, 180, "free", false, false,
        ))
        .expect("free");
    store
        .upsert_calendar_event(&calendar_event(
            "cancelled",
            "Cancelled",
            120,
            180,
            "busy",
            true,
            false,
        ))
        .expect("cancelled");
    store
        .upsert_calendar_event(&calendar_event(
            "deleted", "Deleted", 120, 180, "busy", false, true,
        ))
        .expect("deleted");
    store
        .upsert_calendar_event(&calendar_event(
            "outside", "Outside", 220, 260, "busy", false, false,
        ))
        .expect("outside");

    let adapter = CalendarAdapter::new(NoopCalendarClient);
    let collisions = adapter.collisions(&store, 150, 210).expect("collisions");
    let findings = adapter
        .record_collision_findings(&store, "Planning", 150, 210)
        .expect("findings");

    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0].external_id, "busy");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, "calendar_collision");
}

#[derive(Clone)]
struct MockOutlookClient {
    messages: Vec<NewOutlookMessage>,
    fail_admin_consent: bool,
    sync_calls: Rc<Cell<usize>>,
    send_calls: Rc<Cell<usize>>,
    last_delta: Rc<RefCell<Option<String>>>,
}

impl MockOutlookClient {
    fn with_messages(messages: Vec<NewOutlookMessage>) -> Self {
        Self {
            messages,
            fail_admin_consent: false,
            sync_calls: Rc::new(Cell::new(0)),
            send_calls: Rc::new(Cell::new(0)),
            last_delta: Rc::new(RefCell::new(None)),
        }
    }

    fn failing_admin_consent() -> Self {
        Self {
            fail_admin_consent: true,
            ..Self::with_messages(Vec::new())
        }
    }
}

impl OutlookGraphClient for MockOutlookClient {
    fn sync_mail(
        &self,
        delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewOutlookMessage>, GraphError> {
        self.sync_calls.set(self.sync_calls.get() + 1);
        *self.last_delta.borrow_mut() = delta_link.map(str::to_owned);

        if self.fail_admin_consent {
            return Err(GraphError::AdminConsentRequired {
                message: "AADSTS65001".to_owned(),
            });
        }

        Ok(GraphSyncPage::new(self.messages.clone()).with_delta_link("delta-2"))
    }

    fn send_mail(&self, _draft: &MailDraft) -> Result<ActionReceipt, GraphError> {
        self.send_calls.set(self.send_calls.get() + 1);
        Ok(ActionReceipt::sent("mail-sent-1"))
    }
}

#[derive(Clone, Default)]
struct MockTeamsChatClient {
    send_calls: Rc<Cell<usize>>,
}

impl TeamsChatGraphClient for MockTeamsChatClient {
    fn sync_chat_messages(
        &self,
        _delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewTeamsMessage>, GraphError> {
        Ok(GraphSyncPage::new(Vec::new()).with_delta_link("teams-delta"))
    }

    fn send_chat_message(&self, _draft: &TeamsChatDraft) -> Result<ActionReceipt, GraphError> {
        self.send_calls.set(self.send_calls.get() + 1);
        Ok(ActionReceipt::sent("teams-sent-1"))
    }
}

struct NoopCalendarClient;

impl CalendarGraphClient for NoopCalendarClient {
    fn sync_events(
        &self,
        _delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewCalendarEvent>, GraphError> {
        Ok(GraphSyncPage::new(Vec::new()))
    }

    fn create_event(&self, _draft: &CalendarEventDraft) -> Result<ActionReceipt, GraphError> {
        Ok(ActionReceipt::changed(
            "created",
            Some("event-1".to_owned()),
        ))
    }

    fn update_event(&self, _draft: &CalendarEventDraft) -> Result<ActionReceipt, GraphError> {
        Ok(ActionReceipt::changed(
            "updated",
            Some("event-1".to_owned()),
        ))
    }

    fn delete_event(&self, external_id: &str) -> Result<ActionReceipt, GraphError> {
        Ok(ActionReceipt::changed(
            "deleted",
            Some(external_id.to_owned()),
        ))
    }
}

fn mail_message(external_id: &str) -> NewOutlookMessage {
    NewOutlookMessage {
        external_id: external_id.to_owned(),
        folder_id: Some("inbox".to_owned()),
        subject: Some("Status".to_owned()),
        sender_name: Some("Anna".to_owned()),
        sender_email: Some("anna@example.com".to_owned()),
        body_preview: Some("External message preview".to_owned()),
        received_at: Some(100),
        etag: Some("etag".to_owned()),
        change_key: Some("change".to_owned()),
        is_deleted: false,
    }
}

fn calendar_event(
    external_id: &str,
    subject: &str,
    starts_at: i64,
    ends_at: i64,
    show_as: &str,
    is_cancelled: bool,
    is_deleted: bool,
) -> NewCalendarEvent {
    NewCalendarEvent {
        external_id: external_id.to_owned(),
        subject: Some(subject.to_owned()),
        organizer_name: Some("Anna".to_owned()),
        organizer_email: Some("anna@example.com".to_owned()),
        starts_at: Some(starts_at),
        ends_at: Some(ends_at),
        original_timezone: Some("America/New_York".to_owned()),
        show_as: Some(show_as.to_owned()),
        etag: None,
        change_key: None,
        is_cancelled,
        is_deleted,
    }
}
