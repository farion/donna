use crate::approval::ApprovalDecision;
use crate::microsoft::auth::{MicrosoftTokenSet, load_microsoft_tokens, store_microsoft_tokens};
use crate::microsoft::calendar::{CALENDAR_SOURCE, CalendarAdapter};
use crate::microsoft::error::GraphError;
use crate::microsoft::outlook::{OUTLOOK_MAIL_SOURCE, OutlookAdapter};
use crate::microsoft::teams::{
    TEAMS_CHANNEL_SOURCE, TEAMS_CHAT_SOURCE, TeamsChannelAdapter, TeamsChatAdapter,
};
use crate::secrets::InMemorySecretStore;
use crate::storage::{DataFreshness, LocalStore};

mod mock_graph;
use mock_graph::*;

#[test]
fn microsoft_tokens_round_trip_through_secret_store() {
    let store = InMemorySecretStore::default();
    let tokens = MicrosoftTokenSet {
        access_token: "synthetic-access".to_owned(),
        refresh_token: Some("synthetic-refresh".to_owned()),
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
    let message = store
        .outlook_message_by_external_id("message-1")
        .expect("message");

    assert_eq!(report.synced_records, 1);
    assert_eq!(report.delta_link.as_deref(), Some("delta-2"));
    assert_eq!(message.sender_email.as_deref(), Some("anna@example.com"));
    assert_eq!(message.body_preview.as_deref(), Some(HOSTILE_FIXTURE));
    assert_eq!(
        store
            .sync_state(OUTLOOK_MAIL_SOURCE)
            .expect("state")
            .expect("present")
            .delta_link
            .as_deref(),
        Some("delta-2")
    );
    assert_eq!(
        store
            .data_freshness(OUTLOOK_MAIL_SOURCE)
            .expect("freshness"),
        DataFreshness::Fresh
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
    let draft = mail_draft();

    let error = adapter
        .send_mail(&store, &draft, ApprovalDecision::Pending)
        .expect_err("approval required");

    assert!(matches!(error, GraphError::Approval(_)));
    assert_eq!(client.send_calls.get(), 0);
}

#[test]
fn approved_outlook_send_records_audit_entry() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockOutlookClient::with_messages(Vec::new());
    let adapter = OutlookAdapter::new(client.clone());

    let receipt = adapter
        .send_mail(
            &store,
            &mail_draft(),
            ApprovalDecision::Approved { approved_at: 10 },
        )
        .expect("send");
    let audit = store.audit_entry(1).expect("audit");

    assert_eq!(receipt.external_id.as_deref(), Some("mail-sent-1"));
    assert_eq!(client.send_calls.get(), 1);
    assert_eq!(audit.action_type, "send_mail");
    assert_eq!(audit.target_system, OUTLOOK_MAIL_SOURCE);
    assert_eq!(audit.external_id.as_deref(), Some("mail-sent-1"));
    assert_eq!(audit.result, "sent");
}

#[test]
fn teams_chat_sync_persists_message_and_delta_state() {
    let store = LocalStore::in_memory().expect("store");
    let client =
        MockTeamsChatClient::with_messages(vec![teams_message("chat-message-1", "chat-1")]);
    let adapter = TeamsChatAdapter::new(client.clone());

    let report = adapter.sync_messages(&store).expect("sync chat");
    let message = store
        .teams_message_by_external_id("chat-message-1")
        .expect("message");

    assert_eq!(report.source, TEAMS_CHAT_SOURCE);
    assert_eq!(report.synced_records, 1);
    assert_eq!(report.delta_link.as_deref(), Some("teams-chat-delta"));
    assert_eq!(message.chat_id, "chat-1");
    assert_eq!(message.body, HOSTILE_FIXTURE);
    assert_eq!(
        store
            .sync_state(TEAMS_CHAT_SOURCE)
            .expect("state")
            .expect("present")
            .delta_link
            .as_deref(),
        Some("teams-chat-delta")
    );
    assert_eq!(client.sync_calls.get(), 1);
    assert_eq!(client.last_delta.borrow().as_deref(), None);
}

#[test]
fn pending_teams_chat_approval_does_not_send_message() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockTeamsChatClient::default();
    let adapter = TeamsChatAdapter::new(client.clone());

    let error = adapter
        .send_message(&store, &teams_chat_draft(), ApprovalDecision::Pending)
        .expect_err("approval required");

    assert!(matches!(error, GraphError::Approval(_)));
    assert_eq!(client.send_calls.get(), 0);
}

#[test]
fn approved_teams_chat_send_records_audit_entry() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockTeamsChatClient::default();
    let adapter = TeamsChatAdapter::new(client.clone());

    let receipt = adapter
        .send_message(
            &store,
            &teams_chat_draft(),
            ApprovalDecision::Approved { approved_at: 10 },
        )
        .expect("send");
    let audit = store.audit_entry(1).expect("audit");

    assert_eq!(receipt.external_id.as_deref(), Some("teams-sent-1"));
    assert_eq!(client.send_calls.get(), 1);
    assert_eq!(audit.action_type, "send_teams_message");
    assert_eq!(audit.target_system, TEAMS_CHAT_SOURCE);
    assert_eq!(audit.external_id.as_deref(), Some("teams-sent-1"));
}

#[test]
fn teams_channel_sync_and_send_gates_are_local_and_audited() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockTeamsChannelClient::with_messages(vec![teams_message(
        "channel-message-1",
        "team-1/channel-1",
    )]);
    let adapter = TeamsChannelAdapter::new(client.clone());

    let report = adapter.sync_messages(&store).expect("sync channel");
    let message = store
        .teams_message_by_external_id("channel-message-1")
        .expect("message");

    assert_eq!(report.source, TEAMS_CHANNEL_SOURCE);
    assert_eq!(message.chat_id, "team-1/channel-1");
    assert_eq!(message.body, HOSTILE_FIXTURE);
    assert_eq!(
        store
            .sync_state(TEAMS_CHANNEL_SOURCE)
            .expect("state")
            .expect("present")
            .delta_link
            .as_deref(),
        Some("teams-channel-delta")
    );

    let error = adapter
        .send_message(&store, &teams_channel_draft(), ApprovalDecision::Pending)
        .expect_err("approval required");
    assert!(matches!(error, GraphError::Approval(_)));
    assert_eq!(client.send_calls.get(), 0);

    let receipt = adapter
        .send_message(
            &store,
            &teams_channel_draft(),
            ApprovalDecision::Approved { approved_at: 20 },
        )
        .expect("send channel");
    let audit = store.audit_entry(1).expect("audit");

    assert_eq!(receipt.external_id.as_deref(), Some("teams-channel-sent-1"));
    assert_eq!(client.send_calls.get(), 1);
    assert_eq!(audit.action_type, "send_teams_message");
    assert_eq!(audit.target_system, TEAMS_CHANNEL_SOURCE);
    assert_eq!(audit.external_id.as_deref(), Some("teams-channel-sent-1"));
}

#[test]
fn teams_channel_permission_failure_marks_existing_data_stale() {
    let store = LocalStore::in_memory().expect("store");
    let adapter =
        TeamsChannelAdapter::new(MockTeamsChannelClient::with_messages(vec![teams_message(
            "channel-message-1",
            "team-1/channel-1",
        )]));
    adapter.sync_messages(&store).expect("initial sync");

    let failing = MockTeamsChannelClient::failing_permission();
    let adapter = TeamsChannelAdapter::new(failing);
    let error = adapter
        .sync_messages(&store)
        .expect_err("permission failure");

    assert!(error.is_permission_problem());
    assert_eq!(
        store
            .sync_state(TEAMS_CHANNEL_SOURCE)
            .expect("state")
            .expect("present")
            .delta_link
            .as_deref(),
        Some("teams-channel-delta")
    );
    assert_eq!(
        store
            .data_freshness(TEAMS_CHANNEL_SOURCE)
            .expect("freshness"),
        DataFreshness::Stale {
            error: Some(
                "Microsoft Teams Graph permission is unavailable or not consented: \
                 ChannelMessage.Read.All unavailable in local mock"
                    .to_owned()
            )
        }
    );
}

#[test]
fn calendar_sync_persists_event_and_delta_state() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockCalendarClient::with_events(vec![calendar_event(
        "event-1",
        HOSTILE_FIXTURE,
        300,
        360,
        "busy",
        false,
        false,
    )]);
    let adapter = CalendarAdapter::new(client.clone());

    let report = adapter.sync_events(&store).expect("sync calendar");
    let event = store
        .calendar_event_by_external_id("event-1")
        .expect("event");

    assert_eq!(report.source, CALENDAR_SOURCE);
    assert_eq!(report.synced_records, 1);
    assert_eq!(report.delta_link.as_deref(), Some("calendar-delta"));
    assert_eq!(event.subject.as_deref(), Some(HOSTILE_FIXTURE));
    assert_eq!(
        store
            .sync_state(CALENDAR_SOURCE)
            .expect("state")
            .expect("present")
            .delta_link
            .as_deref(),
        Some("calendar-delta")
    );
    assert_eq!(client.sync_calls.get(), 1);
}

#[test]
fn calendar_create_update_delete_require_approval_and_record_audit() {
    let store = LocalStore::in_memory().expect("store");
    let client = MockCalendarClient::default();
    let adapter = CalendarAdapter::new(client.clone());

    let pending_create = adapter
        .create_event(&store, &calendar_draft(None), ApprovalDecision::Pending)
        .expect_err("create approval");
    assert!(matches!(pending_create, GraphError::Approval(_)));
    assert_eq!(client.create_calls.get(), 0);

    adapter
        .create_event(
            &store,
            &calendar_draft(None),
            ApprovalDecision::Approved { approved_at: 10 },
        )
        .expect("create");
    assert_eq!(client.create_calls.get(), 1);
    assert_audit_entry(
        &store,
        1,
        "create_calendar_event",
        "calendar-created-1",
        "created",
    );

    let pending_update = adapter
        .update_event(
            &store,
            &calendar_draft(Some("event-1")),
            ApprovalDecision::Pending,
        )
        .expect_err("update approval");
    assert!(matches!(pending_update, GraphError::Approval(_)));
    assert_eq!(client.update_calls.get(), 0);

    adapter
        .update_event(
            &store,
            &calendar_draft(Some("event-1")),
            ApprovalDecision::Approved { approved_at: 20 },
        )
        .expect("update");
    assert_eq!(client.update_calls.get(), 1);
    assert_audit_entry(&store, 2, "update_calendar_event", "event-1", "updated");

    let pending_delete = adapter
        .delete_event(&store, "event-1", ApprovalDecision::Pending)
        .expect_err("delete approval");
    assert!(matches!(pending_delete, GraphError::Approval(_)));
    assert_eq!(client.delete_calls.get(), 0);

    adapter
        .delete_event(
            &store,
            "event-1",
            ApprovalDecision::Approved { approved_at: 30 },
        )
        .expect("delete");
    assert_eq!(client.delete_calls.get(), 1);
    assert_audit_entry(&store, 3, "delete_calendar_event", "event-1", "deleted");
}

#[test]
fn offline_calendar_action_does_not_mutate_graph() {
    let store = LocalStore::in_memory().expect("store");
    store.set_offline_mode(true).expect("offline");
    let client = MockCalendarClient::default();
    let adapter = CalendarAdapter::new(client.clone());

    let error = adapter
        .create_event(
            &store,
            &calendar_draft(None),
            ApprovalDecision::Approved { approved_at: 10 },
        )
        .expect_err("offline");

    assert!(matches!(error, GraphError::Offline));
    assert_eq!(client.create_calls.get(), 0);
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

    let adapter = CalendarAdapter::new(MockCalendarClient::default());
    let collisions = adapter.collisions(&store, 150, 210).expect("collisions");
    let findings = adapter
        .record_collision_findings(&store, "Planning", 150, 210)
        .expect("findings");

    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0].external_id, "busy");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].kind, "calendar_collision");
}
