use crate::storage::{DataFreshness, LocalStore};

pub(super) fn status_label(
    base: &str,
    store: Option<&LocalStore>,
    show_stale_warnings: bool,
) -> String {
    let Some(store) = store else {
        return "Storage unavailable".to_owned();
    };

    if store.is_offline().unwrap_or(false) {
        return "Offline".to_owned();
    }

    if show_stale_warnings {
        let stale = [
            ("outlook.mail", "Mail"),
            ("teams.chat", "Teams"),
            ("calendar", "Calendar"),
        ]
        .into_iter()
        .filter_map(|(source, label)| match store.data_freshness(source).ok()? {
            DataFreshness::Stale { .. } => Some(label),
            DataFreshness::Fresh | DataFreshness::NeverSynced => None,
        })
        .collect::<Vec<_>>();

        if !stale.is_empty() {
            return format!("Stale: {}", stale.join(", "));
        }
    }

    base.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NewSyncState;

    #[test]
    fn offline_state_takes_priority_in_status_label() {
        let store = LocalStore::in_memory().expect("store");
        store.set_offline_mode(true).expect("offline");

        assert_eq!(status_label("Idle", Some(&store), true), "Offline");
    }

    #[test]
    fn stale_sync_state_is_visible_in_status_label() {
        let store = LocalStore::in_memory().expect("store");
        store
            .upsert_sync_state(&NewSyncState {
                source: "calendar".to_owned(),
                cursor: None,
                delta_link: None,
                last_sync_at: Some(1_000),
                last_error: Some("network unavailable".to_owned()),
                is_stale: true,
            })
            .expect("sync");

        assert_eq!(status_label("Idle", Some(&store), true), "Stale: Calendar");
    }
}
