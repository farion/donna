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
    pub created_at: i64,
    pub updated_at: i64,
    pub resolved_at: Option<i64>,
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
pub enum DataFreshness {
    Fresh,
    Stale { error: Option<String> },
    NeverSynced,
}
