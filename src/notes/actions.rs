use super::{NoteIndexError, file_stem_title, modified_seconds, parse_markdown_metadata};
use crate::approval::{ApprovalDecision, ApprovalError, ApprovalRequest, ExternalActionKind};
use crate::storage::{AuditEntry, LocalStore, NewNoteMetadata, NoteMetadata, StorageError};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, SystemTimeError, UNIX_EPOCH};

pub const OBSIDIAN_TARGET: &str = "obsidian";

#[derive(Debug)]
pub enum NoteActionError {
    Approval(ApprovalError),
    Clock(SystemTimeError),
    EmptyPath,
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    NoteAlreadyExists(String),
    NoteMissing(String),
    Offline,
    Storage(StorageError),
    UnsafePath(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteWrite {
    pub note_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteEdit {
    pub note_path: String,
    pub contents: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteActionReceipt {
    pub external_id: String,
    pub result: String,
    pub audit_entry: AuditEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsidianNoteAdapter {
    vault_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedNotePath {
    absolute: PathBuf,
    relative: String,
}

impl ObsidianNoteAdapter {
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        Self {
            vault_path: vault_path.into(),
        }
    }

    pub fn prepare_write_note(&self, draft: &NoteWrite) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::WriteNote,
            OBSIDIAN_TARGET,
            format!("Write Obsidian note {}", draft.note_path),
        )
        .with_external_id(&draft.note_path)
    }

    pub fn prepare_edit_note(&self, draft: &NoteEdit) -> ApprovalRequest {
        ApprovalRequest::new(
            ExternalActionKind::EditNote,
            OBSIDIAN_TARGET,
            format!("Edit Obsidian note {}", draft.note_path),
        )
        .with_external_id(&draft.note_path)
    }

    pub fn write_note(
        &self,
        store: &LocalStore,
        draft: &NoteWrite,
        decision: ApprovalDecision,
    ) -> Result<NoteActionReceipt, NoteActionError> {
        ensure_online(store)?;
        let approved = self.prepare_write_note(draft).approve(decision)?;
        let note = self.resolve_note_path(&draft.note_path)?;

        self.ensure_new_note_target_is_not_symlink(&note)?;

        if note.absolute.exists() {
            return Err(NoteActionError::NoteAlreadyExists(note.relative));
        }

        self.ensure_new_note_parent_in_vault(&note)?;

        if let Some(parent) = note.absolute.parent() {
            fs::create_dir_all(parent).map_err(|source| NoteActionError::Io {
                path: parent.to_owned(),
                source,
            })?;
        }

        fs::write(&note.absolute, &draft.contents).map_err(|source| NoteActionError::Io {
            path: note.absolute.clone(),
            source,
        })?;

        record_note_action(store, &approved, &note, "written").and_then(|receipt| {
            self.index_note(store, &note, &draft.contents)?;
            Ok(receipt)
        })
    }

    pub fn edit_note(
        &self,
        store: &LocalStore,
        draft: &NoteEdit,
        decision: ApprovalDecision,
    ) -> Result<NoteActionReceipt, NoteActionError> {
        ensure_online(store)?;
        let approved = self.prepare_edit_note(draft).approve(decision)?;
        let note = self.resolve_note_path(&draft.note_path)?;

        if !note.absolute.is_file() {
            return Err(NoteActionError::NoteMissing(note.relative));
        }

        self.ensure_existing_note_in_vault(&note)?;

        fs::write(&note.absolute, &draft.contents).map_err(|source| NoteActionError::Io {
            path: note.absolute.clone(),
            source,
        })?;

        record_note_action(store, &approved, &note, "edited").and_then(|receipt| {
            self.index_note(store, &note, &draft.contents)?;
            Ok(receipt)
        })
    }

    fn resolve_note_path(&self, note_path: &str) -> Result<ResolvedNotePath, NoteActionError> {
        let trimmed = note_path.trim();
        if trimmed.is_empty() {
            return Err(NoteActionError::EmptyPath);
        }

        if Path::new(trimmed).is_absolute() {
            return Err(NoteActionError::UnsafePath(trimmed.to_owned()));
        }

        let normalized = trimmed.replace('\\', "/");
        let relative_path = Path::new(&normalized);
        let mut clean = PathBuf::new();

        for component in relative_path.components() {
            match component {
                Component::CurDir => {}
                Component::Normal(part) => clean.push(part),
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(NoteActionError::UnsafePath(note_path.to_owned()));
                }
            }
        }

        if clean.as_os_str().is_empty() {
            return Err(NoteActionError::EmptyPath);
        }

        if clean.extension().and_then(|ext| ext.to_str()) != Some("md") {
            return Err(NoteActionError::UnsafePath(note_path.to_owned()));
        }

        Ok(ResolvedNotePath {
            absolute: self.vault_path.join(&clean),
            relative: clean.to_string_lossy().replace('\\', "/"),
        })
    }

    fn ensure_new_note_parent_in_vault(
        &self,
        note: &ResolvedNotePath,
    ) -> Result<(), NoteActionError> {
        let vault = self.canonical_vault_path()?;
        let parent = note.absolute.parent().unwrap_or(&self.vault_path);
        let existing_parent = nearest_existing_ancestor(parent)
            .ok_or_else(|| NoteActionError::UnsafePath(note.relative.clone()))?;
        let parent = canonicalize(existing_parent)?;

        if parent.starts_with(vault) {
            Ok(())
        } else {
            Err(NoteActionError::UnsafePath(note.relative.clone()))
        }
    }

    fn ensure_new_note_target_is_not_symlink(
        &self,
        note: &ResolvedNotePath,
    ) -> Result<(), NoteActionError> {
        match fs::symlink_metadata(&note.absolute) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                Err(NoteActionError::UnsafePath(note.relative.clone()))
            }
            Ok(_) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(NoteActionError::Io {
                path: note.absolute.clone(),
                source,
            }),
        }
    }

    fn ensure_existing_note_in_vault(
        &self,
        note: &ResolvedNotePath,
    ) -> Result<(), NoteActionError> {
        let vault = self.canonical_vault_path()?;
        let note_path = canonicalize(&note.absolute)?;

        if note_path.starts_with(vault) {
            Ok(())
        } else {
            Err(NoteActionError::UnsafePath(note.relative.clone()))
        }
    }

    fn index_note(
        &self,
        store: &LocalStore,
        note: &ResolvedNotePath,
        contents: &str,
    ) -> Result<NoteMetadata, NoteActionError> {
        let parsed = parse_markdown_metadata(contents);
        let title = parsed
            .title
            .or_else(|| file_stem_title(&note.absolute))
            .filter(|value| !value.trim().is_empty());

        Ok(store.upsert_note_metadata(&NewNoteMetadata {
            vault_path: self.vault_path.to_string_lossy().into_owned(),
            note_path: note.relative.clone(),
            title,
            headings: parsed.headings,
            tags: parsed.tags,
            links: parsed.links,
            modified_at: modified_seconds(&note.absolute)?,
        })?)
    }

    fn canonical_vault_path(&self) -> Result<PathBuf, NoteActionError> {
        canonicalize(&self.vault_path)
    }
}

fn ensure_online(store: &LocalStore) -> Result<(), NoteActionError> {
    if store.is_offline()? {
        return Err(NoteActionError::Offline);
    }
    Ok(())
}

fn record_note_action(
    store: &LocalStore,
    approved: &crate::approval::ApprovedAction,
    note: &ResolvedNotePath,
    result: &str,
) -> Result<NoteActionReceipt, NoteActionError> {
    let mut audit = approved.audit_entry(now_seconds()?, result);
    audit.external_id = Some(note.relative.clone());
    let audit_entry = store.record_audit_entry(&audit)?;

    Ok(NoteActionReceipt {
        external_id: note.relative.clone(),
        result: result.to_owned(),
        audit_entry,
    })
}

fn now_seconds() -> Result<i64, NoteActionError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(NoteActionError::Clock)?;
    Ok(elapsed.as_secs() as i64)
}

fn nearest_existing_ancestor(path: &Path) -> Option<&Path> {
    let mut current = path;
    loop {
        if current.exists() {
            return Some(current);
        }
        current = current.parent()?;
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, NoteActionError> {
    fs::canonicalize(path).map_err(|source| NoteActionError::Io {
        path: path.to_owned(),
        source,
    })
}

impl From<ApprovalError> for NoteActionError {
    fn from(error: ApprovalError) -> Self {
        Self::Approval(error)
    }
}

impl From<StorageError> for NoteActionError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<NoteIndexError> for NoteActionError {
    fn from(error: NoteIndexError) -> Self {
        match error {
            NoteIndexError::Io { path, source } => Self::Io { path, source },
            NoteIndexError::Storage(source) => Self::Storage(source),
        }
    }
}

impl Display for NoteActionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteActionError::Approval(source) => write!(formatter, "{source}"),
            NoteActionError::Clock(source) => write!(formatter, "system clock error: {source}"),
            NoteActionError::EmptyPath => write!(formatter, "Obsidian note path cannot be empty"),
            NoteActionError::Io { path, source } => {
                write!(
                    formatter,
                    "Obsidian note IO error at {}: {source}",
                    path.display()
                )
            }
            NoteActionError::NoteAlreadyExists(path) => {
                write!(formatter, "Obsidian note already exists: {path}")
            }
            NoteActionError::NoteMissing(path) => {
                write!(formatter, "Obsidian note is missing: {path}")
            }
            NoteActionError::Offline => write!(
                formatter,
                "Donna is offline; Obsidian note writes and edits are paused"
            ),
            NoteActionError::Storage(source) => write!(formatter, "{source}"),
            NoteActionError::UnsafePath(path) => {
                write!(formatter, "unsafe Obsidian note path: {path}")
            }
        }
    }
}

impl Error for NoteActionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NoteActionError::Approval(source) => Some(source),
            NoteActionError::Clock(source) => Some(source),
            NoteActionError::Io { source, .. } => Some(source),
            NoteActionError::Storage(source) => Some(source),
            NoteActionError::EmptyPath
            | NoteActionError::NoteAlreadyExists(_)
            | NoteActionError::NoteMissing(_)
            | NoteActionError::Offline
            | NoteActionError::UnsafePath(_) => None,
        }
    }
}
