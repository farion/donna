#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphSyncPage<T> {
    pub records: Vec<T>,
    pub cursor: Option<String>,
    pub delta_link: Option<String>,
}

impl<T> GraphSyncPage<T> {
    pub fn new(records: Vec<T>) -> Self {
        Self {
            records,
            cursor: None,
            delta_link: None,
        }
    }

    pub fn with_delta_link(mut self, delta_link: impl Into<String>) -> Self {
        self.delta_link = Some(delta_link.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub source: String,
    pub synced_records: usize,
    pub cursor: Option<String>,
    pub delta_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionReceipt {
    pub external_id: Option<String>,
    pub result: String,
}

impl ActionReceipt {
    pub fn sent(external_id: impl Into<String>) -> Self {
        Self {
            external_id: Some(external_id.into()),
            result: "sent".to_owned(),
        }
    }

    pub fn changed(result: impl Into<String>, external_id: Option<String>) -> Self {
        Self {
            external_id,
            result: result.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailDraft {
    pub to: Vec<String>,
    pub subject: String,
    pub body: String,
}

impl MailDraft {
    pub fn summary(&self) -> String {
        format!(
            "Send mail to {} recipient(s): {}",
            self.to.len(),
            self.subject
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamsChatDraft {
    pub chat_id: String,
    pub body: String,
}

impl TeamsChatDraft {
    pub fn summary(&self) -> String {
        format!("Send Teams chat message to {}", self.chat_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamsChannelDraft {
    pub team_id: String,
    pub channel_id: String,
    pub body: String,
}

impl TeamsChannelDraft {
    pub fn summary(&self) -> String {
        format!(
            "Send Teams channel message to {}/{}",
            self.team_id, self.channel_id
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEventDraft {
    pub external_id: Option<String>,
    pub subject: String,
    pub starts_at: i64,
    pub ends_at: i64,
    pub original_timezone: String,
}

impl CalendarEventDraft {
    pub fn create_summary(&self) -> String {
        format!("Create calendar event: {}", self.subject)
    }

    pub fn update_summary(&self) -> String {
        format!("Update calendar event: {}", self.subject)
    }
}
