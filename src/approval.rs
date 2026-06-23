use crate::storage::NewAuditEntry;
use std::error::Error;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalActionKind {
    SendMail,
    SendTeamsMessage,
    CreateCalendarEvent,
    UpdateCalendarEvent,
    DeleteCalendarEvent,
    WriteNote,
    EditNote,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalDecision {
    Pending,
    Approved { approved_at: i64 },
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequest {
    pub action: ExternalActionKind,
    pub target_system: String,
    pub summary: String,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedAction {
    pub request: ApprovalRequest,
    pub approved_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    ApprovalRequired(ExternalActionKind),
    Rejected(ExternalActionKind),
}

impl ApprovalRequest {
    pub fn new(
        action: ExternalActionKind,
        target_system: impl Into<String>,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            action,
            target_system: target_system.into(),
            summary: summary.into(),
            external_id: None,
        }
    }

    pub fn with_external_id(mut self, external_id: impl Into<String>) -> Self {
        self.external_id = Some(external_id.into());
        self
    }

    pub fn approve(self, decision: ApprovalDecision) -> Result<ApprovedAction, ApprovalError> {
        match decision {
            ApprovalDecision::Approved { approved_at } => Ok(ApprovedAction {
                request: self,
                approved_at,
            }),
            ApprovalDecision::Rejected => Err(ApprovalError::Rejected(self.action)),
            ApprovalDecision::Pending => Err(ApprovalError::ApprovalRequired(self.action)),
        }
    }
}

impl ApprovedAction {
    pub fn audit_entry(&self, execution_at: i64, result: impl Into<String>) -> NewAuditEntry {
        NewAuditEntry {
            action_type: self.request.action.audit_action_type().to_owned(),
            target_system: self.request.target_system.clone(),
            summary: self.request.summary.clone(),
            approval_at: self.approved_at,
            execution_at,
            result: result.into(),
            external_id: self.request.external_id.clone(),
        }
    }
}

impl ExternalActionKind {
    fn audit_action_type(self) -> &'static str {
        match self {
            ExternalActionKind::SendMail => "send_mail",
            ExternalActionKind::SendTeamsMessage => "send_teams_message",
            ExternalActionKind::CreateCalendarEvent => "create_calendar_event",
            ExternalActionKind::UpdateCalendarEvent => "update_calendar_event",
            ExternalActionKind::DeleteCalendarEvent => "delete_calendar_event",
            ExternalActionKind::WriteNote => "write_note",
            ExternalActionKind::EditNote => "edit_note",
        }
    }
}

impl Display for ApprovalError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalError::ApprovalRequired(action) => {
                write!(formatter, "{action:?} requires explicit approval")
            }
            ApprovalError::Rejected(action) => write!(formatter, "{action:?} was rejected"),
        }
    }
}

impl Error for ApprovalError {}

#[cfg(test)]
mod tests {
    use super::{ApprovalDecision, ApprovalError, ApprovalRequest, ExternalActionKind};

    #[test]
    fn external_actions_cannot_be_approved_implicitly() {
        let request =
            ApprovalRequest::new(ExternalActionKind::SendMail, "outlook", "Reply to billing");

        let error = request
            .approve(ApprovalDecision::Pending)
            .expect_err("must require approval");

        assert_eq!(
            error,
            ApprovalError::ApprovalRequired(ExternalActionKind::SendMail)
        );
    }

    #[test]
    fn approved_action_builds_audit_record_without_secret_fields() {
        let request = ApprovalRequest::new(
            ExternalActionKind::UpdateCalendarEvent,
            "calendar",
            "Move planning meeting",
        )
        .with_external_id("event-1");

        let action = request
            .approve(ApprovalDecision::Approved { approved_at: 10 })
            .expect("approved");
        let audit = action.audit_entry(12, "updated");

        assert_eq!(audit.action_type, "update_calendar_event");
        assert_eq!(audit.target_system, "calendar");
        assert_eq!(audit.external_id.as_deref(), Some("event-1"));
        assert!(!audit.summary.contains("token"));
    }
}
