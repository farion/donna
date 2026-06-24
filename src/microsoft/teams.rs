use crate::approval::{ApprovalDecision, ApprovalRequest, ExternalActionKind};
use crate::microsoft::error::GraphError;
use crate::microsoft::sync::run_sync;
use crate::microsoft::types::{
    ActionReceipt, GraphSyncPage, SyncReport, TeamsChannelDraft, TeamsChatDraft,
};
use crate::storage::{LocalStore, NewTeamsMessage};
use std::time::{SystemTime, UNIX_EPOCH};

pub const TEAMS_CHAT_SOURCE: &str = "teams.chat";
pub const TEAMS_CHANNEL_SOURCE: &str = "teams.channel";

pub trait TeamsChatGraphClient {
    fn sync_chat_messages(
        &self,
        delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewTeamsMessage>, GraphError>;

    fn send_chat_message(&self, draft: &TeamsChatDraft) -> Result<ActionReceipt, GraphError>;
}

pub trait TeamsChannelGraphClient {
    fn sync_channel_messages(
        &self,
        delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewTeamsMessage>, GraphError>;

    fn send_channel_message(&self, draft: &TeamsChannelDraft) -> Result<ActionReceipt, GraphError>;
}

#[derive(Debug, Clone)]
pub struct TeamsChatAdapter<C> {
    client: C,
}

#[derive(Debug, Clone)]
pub struct TeamsChannelAdapter<C> {
    client: C,
}

impl<C> TeamsChatAdapter<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn prepare_send_message(&self, draft: &TeamsChatDraft) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::SendTeamsMessage,
            TEAMS_CHAT_SOURCE,
            draft.summary(),
        )
        .with_external_id(&draft.chat_id)
    }
}

impl<C> TeamsChannelAdapter<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn prepare_send_message(&self, draft: &TeamsChannelDraft) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::SendTeamsMessage,
            TEAMS_CHANNEL_SOURCE,
            draft.summary(),
        )
        .with_external_id(format!("{}/{}", draft.team_id, draft.channel_id))
    }
}

impl<C> TeamsChatAdapter<C>
where
    C: TeamsChatGraphClient,
{
    pub fn sync_messages(&self, store: &LocalStore) -> Result<SyncReport, GraphError> {
        run_sync(
            store,
            TEAMS_CHAT_SOURCE,
            |delta_link| self.client.sync_chat_messages(delta_link),
            |message| store.upsert_teams_message(message).map(|_| ()),
        )
    }

    pub fn send_message(
        &self,
        store: &LocalStore,
        draft: &TeamsChatDraft,
        decision: ApprovalDecision,
    ) -> Result<ActionReceipt, GraphError> {
        ensure_online(store)?;
        let approved = self.prepare_send_message(draft).approve(decision)?;
        let receipt = self.client.send_chat_message(draft)?;
        record_action(store, approved, receipt)
    }
}

impl<C> TeamsChannelAdapter<C>
where
    C: TeamsChannelGraphClient,
{
    pub fn sync_messages(&self, store: &LocalStore) -> Result<SyncReport, GraphError> {
        run_sync(
            store,
            TEAMS_CHANNEL_SOURCE,
            |delta_link| self.client.sync_channel_messages(delta_link),
            |message| store.upsert_teams_message(message).map(|_| ()),
        )
    }

    pub fn send_message(
        &self,
        store: &LocalStore,
        draft: &TeamsChannelDraft,
        decision: ApprovalDecision,
    ) -> Result<ActionReceipt, GraphError> {
        ensure_online(store)?;
        let approved = self.prepare_send_message(draft).approve(decision)?;
        let receipt = self.client.send_channel_message(draft)?;
        record_action(store, approved, receipt)
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
