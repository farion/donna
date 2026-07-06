pub mod auth;
pub mod calendar;
pub mod error;
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
