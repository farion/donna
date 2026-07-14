pub mod auth;
pub mod background_sync;
pub mod calendar;
pub mod error;
mod graph_client;
pub mod outlook;
mod sync;
pub mod teams;
pub mod types;

pub use error::GraphError;
pub use types::{
    ActionReceipt, CalendarEventDraft, GraphSyncPage, MailDraft, SyncReport, TeamsChannelDraft,
    TeamsChatDraft,
};

#[cfg(test)]
mod tests;
