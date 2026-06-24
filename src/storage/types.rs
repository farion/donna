#[derive(Debug, Clone, PartialEq)]
pub struct NewMemory {
    pub memory_type: String,
    pub content: String,
    pub source: String,
    pub confidence: f64,
    pub importance: i64,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredMemory {
    pub id: i64,
    pub memory_type: String,
    pub content: String,
    pub source: String,
    pub confidence: f64,
    pub importance: i64,
    pub created_at: i64,
    pub updated_at: i64,
    pub expires_at: Option<i64>,
    pub forgotten_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewTodo {
    pub title: String,
    pub notes: Option<String>,
    pub source: String,
    pub related_topic: Option<String>,
    pub due_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredTodo {
    pub id: i64,
    pub title: String,
    pub notes: Option<String>,
    pub status: String,
    pub source: String,
    pub related_topic: Option<String>,
    pub due_at: Option<i64>,
    pub snoozed_until: Option<i64>,
    pub stale_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub completed_at: Option<i64>,
    pub dismissed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPerson {
    pub display_name: String,
    pub aliases: Vec<String>,
    pub emails: Vec<String>,
    pub teams_ids: Vec<String>,
    pub context: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: i64,
    pub display_name: String,
    pub aliases: Vec<String>,
    pub emails: Vec<String>,
    pub teams_ids: Vec<String>,
    pub context: Option<String>,
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewFollowUp {
    pub direction: String,
    pub person_id: Option<i64>,
    pub source: String,
    pub summary: String,
    pub due_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FollowUp {
    pub id: i64,
    pub direction: String,
    pub person_id: Option<i64>,
    pub status: String,
    pub source: String,
    pub summary: String,
    pub due_at: Option<i64>,
    pub stale_at: Option<i64>,
    pub snoozed_until: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub resolved_at: Option<i64>,
    pub dismissed_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNoteMetadata {
    pub vault_path: String,
    pub note_path: String,
    pub title: Option<String>,
    pub headings: Vec<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub modified_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteMetadata {
    pub id: i64,
    pub vault_path: String,
    pub note_path: String,
    pub title: Option<String>,
    pub headings: Vec<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
    pub modified_at: Option<i64>,
    pub indexed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSyncState {
    pub source: String,
    pub cursor: Option<String>,
    pub delta_link: Option<String>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub is_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncState {
    pub source: String,
    pub cursor: Option<String>,
    pub delta_link: Option<String>,
    pub last_sync_at: Option<i64>,
    pub last_error: Option<String>,
    pub is_stale: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewOutlookMessage {
    pub external_id: String,
    pub folder_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: Option<String>,
    pub body_preview: Option<String>,
    pub received_at: Option<i64>,
    pub etag: Option<String>,
    pub change_key: Option<String>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutlookMessage {
    pub id: i64,
    pub external_id: String,
    pub folder_id: Option<String>,
    pub subject: Option<String>,
    pub sender_name: Option<String>,
    pub sender_email: Option<String>,
    pub body_preview: Option<String>,
    pub received_at: Option<i64>,
    pub synced_at: i64,
    pub etag: Option<String>,
    pub change_key: Option<String>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTeamsMessage {
    pub external_id: String,
    pub chat_id: String,
    pub sender_name: Option<String>,
    pub sender_external_id: Option<String>,
    pub body: String,
    pub importance: Option<String>,
    pub web_url: Option<String>,
    pub sent_at: Option<i64>,
    pub etag: Option<String>,
    pub change_key: Option<String>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamsMessage {
    pub id: i64,
    pub external_id: String,
    pub chat_id: String,
    pub sender_name: Option<String>,
    pub sender_external_id: Option<String>,
    pub body: String,
    pub importance: Option<String>,
    pub web_url: Option<String>,
    pub sent_at: Option<i64>,
    pub synced_at: i64,
    pub etag: Option<String>,
    pub change_key: Option<String>,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCalendarEvent {
    pub external_id: String,
    pub subject: Option<String>,
    pub organizer_name: Option<String>,
    pub organizer_email: Option<String>,
    pub starts_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub original_timezone: Option<String>,
    pub show_as: Option<String>,
    pub etag: Option<String>,
    pub change_key: Option<String>,
    pub is_cancelled: bool,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarEvent {
    pub id: i64,
    pub external_id: String,
    pub subject: Option<String>,
    pub organizer_name: Option<String>,
    pub organizer_email: Option<String>,
    pub starts_at: Option<i64>,
    pub ends_at: Option<i64>,
    pub original_timezone: Option<String>,
    pub show_as: Option<String>,
    pub synced_at: i64,
    pub etag: Option<String>,
    pub change_key: Option<String>,
    pub is_cancelled: bool,
    pub is_deleted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTaskRun {
    pub task_id: String,
    pub task_model_id: String,
    pub status: String,
    pub prompt_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRun {
    pub id: i64,
    pub task_id: String,
    pub task_model_id: String,
    pub status: String,
    pub prompt_path: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub error_summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTaskFinding {
    pub task_run_id: Option<i64>,
    pub kind: String,
    pub summary: String,
    pub source: String,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskFinding {
    pub id: i64,
    pub task_run_id: Option<i64>,
    pub kind: String,
    pub summary: String,
    pub source: String,
    pub created_at: i64,
    pub dismissed_at: Option<i64>,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAttentionItem {
    pub source_type: String,
    pub source_id: Option<i64>,
    pub level: String,
    pub title: String,
    pub body: Option<String>,
    pub due_at: Option<i64>,
    pub payload: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttentionItem {
    pub id: i64,
    pub source_type: String,
    pub source_id: Option<i64>,
    pub level: String,
    pub title: String,
    pub body: Option<String>,
    pub status: String,
    pub due_at: Option<i64>,
    pub snoozed_until: Option<i64>,
    pub dismissed_at: Option<i64>,
    pub completed_at: Option<i64>,
    pub feedback: Option<String>,
    pub payload: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchContentTrust {
    LocalStructuredData,
    ExternalUntrustedData,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    pub text: String,
    pub record_types: Vec<String>,
    pub source: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResult {
    pub record_type: String,
    pub record_id: i64,
    pub title: String,
    pub snippet: String,
    pub source: String,
    pub trust: SearchContentTrust,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataFreshness {
    Fresh,
    Stale { error: Option<String> },
    NeverSynced,
}

impl SearchQuery {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            record_types: Vec::new(),
            source: None,
            limit: 20,
        }
    }
}
