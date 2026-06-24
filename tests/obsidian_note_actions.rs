use donna::approval::{ApprovalDecision, ApprovalError, ExternalActionKind};
use donna::notes::{NoteActionError, NoteEdit, NoteWrite, ObsidianNoteAdapter};
use donna::storage::LocalStore;

fn approved_at(seconds: i64) -> ApprovalDecision {
    ApprovalDecision::Approved {
        approved_at: seconds,
    }
}

#[test]
fn pending_write_does_not_create_note_or_audit_entry() {
    let dir = tempfile::tempdir().expect("vault");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(dir.path());
    let draft = NoteWrite {
        note_path: "daily/today.md".to_owned(),
        contents: "# Today\n\nPrivate note body.".to_owned(),
    };

    let error = adapter
        .write_note(&store, &draft, ApprovalDecision::Pending)
        .expect_err("approval required");

    assert!(matches!(
        error,
        NoteActionError::Approval(ApprovalError::ApprovalRequired(
            ExternalActionKind::WriteNote
        ))
    ));
    assert!(!dir.path().join("daily/today.md").exists());
    assert!(store.audit_entry(1).is_err());
}

#[test]
fn approved_write_creates_note_metadata_and_audit_entry() {
    let dir = tempfile::tempdir().expect("vault");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(dir.path());
    let draft = NoteWrite {
        note_path: "daily/today.md".to_owned(),
        contents: "# Today\n\nDiscuss [[Plan]] #work.".to_owned(),
    };

    let receipt = adapter
        .write_note(&store, &draft, approved_at(42))
        .expect("write note");

    assert_eq!(receipt.external_id, "daily/today.md");
    assert_eq!(receipt.result, "written");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("daily/today.md")).expect("read note"),
        draft.contents
    );

    let metadata = store
        .note_metadata(&dir.path().to_string_lossy(), "daily/today.md")
        .expect("read metadata")
        .expect("metadata exists");
    assert_eq!(metadata.title.as_deref(), Some("Today"));
    assert_eq!(metadata.headings, vec!["Today"]);
    assert_eq!(metadata.tags, vec!["work"]);
    assert_eq!(metadata.links, vec!["Plan"]);

    let audit = receipt.audit_entry;
    assert_eq!(audit.action_type, "write_note");
    assert_eq!(audit.target_system, "obsidian");
    assert_eq!(audit.summary, "Write Obsidian note daily/today.md");
    assert_eq!(audit.approval_at, 42);
    assert!(audit.execution_at >= audit.approval_at);
    assert_eq!(audit.result, "written");
    assert_eq!(audit.external_id.as_deref(), Some("daily/today.md"));
}

#[test]
fn approved_edit_replaces_existing_note_and_records_edit_audit() {
    let dir = tempfile::tempdir().expect("vault");
    let note_path = dir.path().join("project.md");
    std::fs::write(&note_path, "# Old\n").expect("seed note");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(dir.path());
    let draft = NoteEdit {
        note_path: "project.md".to_owned(),
        contents: "# Project\n\nUpdated #status".to_owned(),
    };

    let receipt = adapter
        .edit_note(&store, &draft, approved_at(50))
        .expect("edit note");

    assert_eq!(
        std::fs::read_to_string(&note_path).expect("read note"),
        draft.contents
    );
    let metadata = store
        .note_metadata(&dir.path().to_string_lossy(), "project.md")
        .expect("read metadata")
        .expect("metadata exists");
    assert_eq!(metadata.title.as_deref(), Some("Project"));
    assert_eq!(metadata.tags, vec!["status"]);

    let audit = receipt.audit_entry;
    assert_eq!(audit.action_type, "edit_note");
    assert_eq!(audit.target_system, "obsidian");
    assert_eq!(audit.summary, "Edit Obsidian note project.md");
    assert_eq!(audit.approval_at, 50);
    assert_eq!(audit.result, "edited");
    assert_eq!(audit.external_id.as_deref(), Some("project.md"));
}

#[test]
fn offline_write_is_rejected_before_touching_files() {
    let dir = tempfile::tempdir().expect("vault");
    let store = LocalStore::in_memory().expect("store");
    store.set_offline_mode(true).expect("offline");
    let adapter = ObsidianNoteAdapter::new(dir.path());
    let draft = NoteWrite {
        note_path: "offline.md".to_owned(),
        contents: "# Offline\n".to_owned(),
    };

    let error = adapter
        .write_note(&store, &draft, approved_at(1))
        .expect_err("offline");

    assert!(matches!(error, NoteActionError::Offline));
    assert!(!dir.path().join("offline.md").exists());
    assert!(store.audit_entry(1).is_err());
}

#[test]
fn unsafe_note_paths_are_rejected() {
    let dir = tempfile::tempdir().expect("vault");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(dir.path());
    let draft = NoteWrite {
        note_path: "../escape.md".to_owned(),
        contents: "# Escape\n".to_owned(),
    };

    let error = adapter
        .write_note(&store, &draft, approved_at(1))
        .expect_err("unsafe path");

    assert!(matches!(error, NoteActionError::UnsafePath(_)));
    assert!(!dir.path().join("../escape.md").exists());
    assert!(store.audit_entry(1).is_err());
}

#[test]
fn write_refuses_to_overwrite_existing_note() {
    let dir = tempfile::tempdir().expect("vault");
    std::fs::write(dir.path().join("existing.md"), "# Existing\n").expect("seed note");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(dir.path());
    let draft = NoteWrite {
        note_path: "existing.md".to_owned(),
        contents: "# Replacement\n".to_owned(),
    };

    let error = adapter
        .write_note(&store, &draft, approved_at(1))
        .expect_err("existing note");

    assert!(matches!(error, NoteActionError::NoteAlreadyExists(path) if path == "existing.md"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("existing.md")).expect("read note"),
        "# Existing\n"
    );
    assert!(store.audit_entry(1).is_err());
}

#[cfg(unix)]
#[test]
fn write_rejects_broken_symlink_note_that_points_outside_vault() {
    let vault = tempfile::tempdir().expect("vault");
    let outside = tempfile::tempdir().expect("outside");
    let outside_target = outside.path().join("created-outside.md");
    std::os::unix::fs::symlink(&outside_target, vault.path().join("linked.md")).expect("symlink");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(vault.path());
    let draft = NoteWrite {
        note_path: "linked.md".to_owned(),
        contents: "# Escaped\n".to_owned(),
    };

    let error = adapter
        .write_note(&store, &draft, approved_at(1))
        .expect_err("symlink escape");

    assert!(matches!(error, NoteActionError::UnsafePath(path) if path == "linked.md"));
    assert!(!outside_target.exists());
    assert!(store.audit_entry(1).is_err());
}

#[cfg(unix)]
#[test]
fn write_rejects_symlink_parent_that_resolves_outside_vault() {
    let vault = tempfile::tempdir().expect("vault");
    let outside = tempfile::tempdir().expect("outside");
    std::os::unix::fs::symlink(outside.path(), vault.path().join("linked")).expect("symlink");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(vault.path());
    let draft = NoteWrite {
        note_path: "linked/new.md".to_owned(),
        contents: "# Outside\n".to_owned(),
    };

    let error = adapter
        .write_note(&store, &draft, approved_at(1))
        .expect_err("symlink escape");

    assert!(matches!(error, NoteActionError::UnsafePath(path) if path == "linked/new.md"));
    assert!(!outside.path().join("new.md").exists());
    assert!(store.audit_entry(1).is_err());
}

#[cfg(unix)]
#[test]
fn edit_rejects_symlink_file_that_resolves_outside_vault() {
    let vault = tempfile::tempdir().expect("vault");
    let outside = tempfile::tempdir().expect("outside");
    let outside_note = outside.path().join("outside.md");
    std::fs::write(&outside_note, "# Outside\n").expect("seed outside note");
    std::os::unix::fs::symlink(&outside_note, vault.path().join("linked.md")).expect("symlink");
    let store = LocalStore::in_memory().expect("store");
    let adapter = ObsidianNoteAdapter::new(vault.path());
    let draft = NoteEdit {
        note_path: "linked.md".to_owned(),
        contents: "# Escaped\n".to_owned(),
    };

    let error = adapter
        .edit_note(&store, &draft, approved_at(1))
        .expect_err("symlink escape");

    assert!(matches!(error, NoteActionError::UnsafePath(path) if path == "linked.md"));
    assert_eq!(
        std::fs::read_to_string(outside_note).expect("read outside note"),
        "# Outside\n"
    );
    assert!(store.audit_entry(1).is_err());
}
