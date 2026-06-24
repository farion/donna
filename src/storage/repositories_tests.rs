use super::*;

#[test]
fn stores_basic_records_and_state() {
    let store = LocalStore::in_memory().expect("store");

    let memory = store
        .create_memory(&NewMemory {
            memory_type: "preference".to_owned(),
            content: "Prefers concise morning planning.".to_owned(),
            source: "donna_chat".to_owned(),
            confidence: 0.8,
            importance: 5,
            expires_at: None,
        })
        .expect("create memory");
    assert_eq!(
        store.memory(memory.id).expect("memory").content,
        memory.content
    );

    let todo = store
        .create_todo(&NewTodo {
            title: "Draft billing retry concept".to_owned(),
            notes: Some("From meeting follow-up".to_owned()),
            source: "donna_chat".to_owned(),
            related_topic: Some("billing".to_owned()),
            due_at: Some(2_000),
        })
        .expect("create todo");
    assert_eq!(todo.status, "open");
    assert_eq!(
        store
            .update_todo_status(todo.id, "done")
            .expect("complete todo")
            .status,
        "done"
    );

    let person = store
        .create_person(&NewPerson {
            display_name: "Anna Example".to_owned(),
            aliases: vec!["Anna".to_owned()],
            emails: vec!["anna@example.com".to_owned()],
            teams_ids: vec!["teams-anna".to_owned()],
            context: Some("Billing partner".to_owned()),
            source: "outlook".to_owned(),
        })
        .expect("create person");
    assert_eq!(person.aliases, vec!["Anna"]);
    assert_eq!(person.emails, vec!["anna@example.com"]);

    let follow_up = store
        .create_follow_up(&NewFollowUp {
            direction: "waiting_for_me".to_owned(),
            person_id: Some(person.id),
            source: "teams".to_owned(),
            summary: "Anna is waiting for billing retry answer.".to_owned(),
            due_at: None,
        })
        .expect("create follow-up");
    assert_eq!(follow_up.person_id, Some(person.id));

    let sync_state = store
        .upsert_sync_state(&NewSyncState {
            source: "outlook.mail".to_owned(),
            cursor: None,
            delta_link: Some("delta-token".to_owned()),
            last_sync_at: Some(1_000),
            last_error: None,
            is_stale: false,
        })
        .expect("sync state");
    assert_eq!(sync_state.delta_link.as_deref(), Some("delta-token"));
    assert_eq!(
        store.data_freshness("outlook.mail").expect("fresh"),
        DataFreshness::Fresh
    );

    store.set_offline_mode(true).expect("set offline");
    assert!(store.is_offline().expect("offline flag"));
}

#[test]
fn stale_sync_state_is_visible() {
    let store = LocalStore::in_memory().expect("store");

    store
        .upsert_sync_state(&NewSyncState {
            source: "teams.chat".to_owned(),
            cursor: None,
            delta_link: None,
            last_sync_at: Some(1_000),
            last_error: Some("network unavailable".to_owned()),
            is_stale: true,
        })
        .expect("sync state");

    assert_eq!(
        store.data_freshness("teams.chat").expect("freshness"),
        DataFreshness::Stale {
            error: Some("network unavailable".to_owned())
        }
    );
    assert_eq!(
        store.data_freshness("calendar").expect("missing freshness"),
        DataFreshness::NeverSynced
    );
}

#[test]
fn corrects_memory_content_and_removes_forgotten_memory_from_search() {
    let store = LocalStore::in_memory().expect("store");
    let memory = store
        .create_memory(&NewMemory {
            memory_type: "fact".to_owned(),
            content: "Fact: original sensitive credential note".to_owned(),
            source: "donna_chat".to_owned(),
            confidence: 0.8,
            importance: 1,
            expires_at: None,
        })
        .expect("create memory");

    let corrected = store
        .update_memory_content(memory.id, "Fact: corrected private vault reference")
        .expect("correct memory");

    assert_eq!(corrected.content, "Fact: corrected private vault reference");
    assert!(
        store
            .search(&SearchQuery::text("original"))
            .expect("search original")
            .is_empty()
    );
    assert_eq!(
        store
            .search(&SearchQuery::text("corrected"))
            .expect("search corrected")[0]
            .record_id,
        memory.id
    );

    let forgotten = store.forget_memory(memory.id).expect("forget memory");

    assert!(forgotten.forgotten_at.is_some());
    assert!(
        store
            .search(&SearchQuery::text("corrected"))
            .expect("search corrected after forget")
            .is_empty()
    );
}
