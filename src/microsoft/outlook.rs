use crate::approval::{ApprovalDecision, ApprovalRequest, ExternalActionKind};
use crate::microsoft::error::GraphError;
use crate::microsoft::sync::run_sync;
use crate::microsoft::types::{ActionReceipt, GraphSyncPage, MailDraft, SyncReport};
use crate::storage::{LocalStore, NewOutlookMessage};
use std::time::{SystemTime, UNIX_EPOCH};

pub const OUTLOOK_MAIL_SOURCE: &str = "outlook.mail";

pub trait OutlookGraphClient {
    fn sync_mail(
        &self,
        delta_link: Option<&str>,
    ) -> Result<GraphSyncPage<NewOutlookMessage>, GraphError>;

    fn send_mail(&self, draft: &MailDraft) -> Result<ActionReceipt, GraphError>;
}

#[derive(Debug, Clone)]
pub struct OutlookAdapter<C> {
    client: C,
}

impl<C> OutlookAdapter<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }

    pub fn prepare_send_mail(&self, draft: &MailDraft) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::SendMail,
            OUTLOOK_MAIL_SOURCE,
            draft.summary(),
        )
    }
}

impl<C> OutlookAdapter<C>
where
    C: OutlookGraphClient,
{
    pub fn sync_mail(&self, store: &LocalStore) -> Result<SyncReport, GraphError> {
        run_sync(
            store,
            OUTLOOK_MAIL_SOURCE,
            |delta_link| self.client.sync_mail(delta_link),
            |message| store.upsert_outlook_message(message).map(|_| ()),
        )
    }

    pub fn send_mail(
        &self,
        store: &LocalStore,
        draft: &MailDraft,
        decision: ApprovalDecision,
    ) -> Result<ActionReceipt, GraphError> {
        ensure_online(store)?;
        let approved = self.prepare_send_mail(draft).approve(decision)?;
        let receipt = self.client.send_mail(draft)?;
        let external_id = receipt.external_id.clone();

        let mut audit = approved.audit_entry(now_seconds()?, receipt.result.clone());
        audit.external_id = external_id;
        store.record_audit_entry(&audit)?;

        Ok(receipt)
    }
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
