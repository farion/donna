use crate::microsoft::error::GraphError;
use crate::microsoft::types::{GraphSyncPage, SyncReport};
use crate::storage::{LocalStore, NewSyncState, StorageError};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_sync<T>(
    store: &LocalStore,
    source: &str,
    fetch: impl FnOnce(Option<&str>) -> Result<GraphSyncPage<T>, GraphError>,
    mut persist: impl FnMut(&T) -> Result<(), StorageError>,
) -> Result<SyncReport, GraphError> {
    if store.is_offline()? {
        mark_stale(store, source, "offline")?;
        return Err(GraphError::Offline);
    }

    let previous = store.sync_state(source)?;
    let previous_delta = previous.as_ref().and_then(|state| state.delta_link.clone());

    match fetch(previous_delta.as_deref()) {
        Ok(page) => {
            for record in &page.records {
                persist(record)?;
            }

            let synced_records = page.records.len();
            store.upsert_sync_state(&NewSyncState {
                source: source.to_owned(),
                cursor: page.cursor.clone(),
                delta_link: page.delta_link.clone(),
                last_sync_at: Some(now_seconds()?),
                last_error: None,
                is_stale: false,
            })?;

            Ok(SyncReport {
                source: source.to_owned(),
                synced_records,
                cursor: page.cursor,
                delta_link: page.delta_link,
            })
        }
        Err(error) => {
            mark_stale(store, source, &error.sync_error_message())?;
            Err(error)
        }
    }
}

fn mark_stale(store: &LocalStore, source: &str, error: &str) -> Result<(), GraphError> {
    let previous = store.sync_state(source)?;
    let cursor = previous.as_ref().and_then(|state| state.cursor.clone());
    let delta_link = previous.as_ref().and_then(|state| state.delta_link.clone());
    let last_sync_at = previous.as_ref().and_then(|state| state.last_sync_at);
    store.upsert_sync_state(&NewSyncState {
        source: source.to_owned(),
        cursor,
        delta_link,
        last_sync_at,
        last_error: Some(error.to_owned()),
        is_stale: true,
    })?;
    Ok(())
}

fn now_seconds() -> Result<i64, GraphError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(elapsed.as_secs() as i64)
}
