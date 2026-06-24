use crate::storage::{LocalStore, NewNoteMetadata, NoteMetadata, StorageError};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub enum NoteIndexError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Storage(StorageError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedNoteMetadata {
    pub title: Option<String>,
    pub headings: Vec<String>,
    pub tags: Vec<String>,
    pub links: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsidianIndexer {
    vault_path: PathBuf,
}

impl ObsidianIndexer {
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        Self {
            vault_path: vault_path.into(),
        }
    }

    pub fn index_vault(&self, store: &LocalStore) -> Result<Vec<NoteMetadata>, NoteIndexError> {
        let mut indexed = Vec::new();
        for path in markdown_files(&self.vault_path)? {
            let contents = read_to_string(&path)?;
            let parsed = parse_markdown_metadata(&contents);
            let note_path = path
                .strip_prefix(&self.vault_path)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let title = parsed
                .title
                .or_else(|| file_stem_title(&path))
                .filter(|value| !value.trim().is_empty());
            let modified_at = modified_seconds(&path)?;

            indexed.push(store.upsert_note_metadata(&NewNoteMetadata {
                vault_path: self.vault_path.to_string_lossy().into_owned(),
                note_path,
                title,
                headings: parsed.headings,
                tags: parsed.tags,
                links: parsed.links,
                modified_at,
            })?);
        }

        Ok(indexed)
    }
}

pub fn parse_markdown_metadata(contents: &str) -> ParsedNoteMetadata {
    let mut headings = Vec::new();
    let mut tags = BTreeSet::new();
    let mut links = BTreeSet::new();

    for line in contents.lines() {
        if let Some(heading) = parse_heading(line) {
            headings.push(heading);
        }
        tags.extend(parse_tags(line));
        links.extend(parse_wiki_links(line));
        links.extend(parse_markdown_links(line));
    }

    ParsedNoteMetadata {
        title: headings.first().cloned(),
        headings,
        tags: tags.into_iter().collect(),
        links: links.into_iter().collect(),
    }
}

fn markdown_files(root: &Path) -> Result<Vec<PathBuf>, NoteIndexError> {
    let mut paths = Vec::new();
    collect_markdown_files(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}

fn collect_markdown_files(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), NoteIndexError> {
    for entry in fs::read_dir(path).map_err(|source| NoteIndexError::Io {
        path: path.to_owned(),
        source,
    })? {
        let entry = entry.map_err(|source| NoteIndexError::Io {
            path: path.to_owned(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, out)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            out.push(path);
        }
    }
    Ok(())
}

fn read_to_string(path: &Path) -> Result<String, NoteIndexError> {
    fs::read_to_string(path).map_err(|source| NoteIndexError::Io {
        path: path.to_owned(),
        source,
    })
}

fn modified_seconds(path: &Path) -> Result<Option<i64>, NoteIndexError> {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| NoteIndexError::Io {
            path: path.to_owned(),
            source,
        })?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs() as i64))
}

fn parse_heading(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if level == 0 || level > 6 {
        return None;
    }

    let marker_tail = &trimmed[level..];
    if !marker_tail.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }

    let heading = marker_tail.trim_start();
    if heading.is_empty() {
        None
    } else {
        Some(heading.trim().to_owned())
    }
}

fn parse_tags(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter_map(|token| token.strip_prefix('#'))
        .map(|tag| tag.trim_matches(|ch: char| !tag_char(ch)))
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn parse_wiki_links(line: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("[[") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find("]]") else {
            break;
        };
        let target = rest[..end].split(['|', '#']).next().unwrap_or("").trim();
        if !target.is_empty() {
            links.push(target.to_owned());
        }
        rest = &rest[end + 2..];
    }
    links
}

fn parse_markdown_links(line: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("](") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let target = rest[..end].trim();
        if !target.is_empty() {
            links.push(target.to_owned());
        }
        rest = &rest[end + 1..];
    }
    links
}

fn tag_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '/')
}

fn file_stem_title(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_owned)
}

impl From<StorageError> for NoteIndexError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl Display for NoteIndexError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NoteIndexError::Io { path, source } => {
                write!(
                    formatter,
                    "note index IO error at {}: {source}",
                    path.display()
                )
            }
            NoteIndexError::Storage(source) => write!(formatter, "{source}"),
        }
    }
}

impl Error for NoteIndexError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            NoteIndexError::Io { source, .. } => Some(source),
            NoteIndexError::Storage(source) => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{SearchContentTrust, SearchQuery};

    #[test]
    fn parses_metadata_without_summarizing_note_body() {
        let parsed = parse_markdown_metadata(
            "# Billing\n\nDiscuss #finance with [[Anna Notes|Anna]].\nRaw body stays local.",
        );

        assert_eq!(parsed.title.as_deref(), Some("Billing"));
        assert_eq!(parsed.tags, vec!["finance"]);
        assert_eq!(parsed.links, vec!["Anna Notes"]);
    }

    #[test]
    fn indexes_metadata_only_into_search() {
        let dir = tempfile::tempdir().expect("dir");
        let note_path = dir.path().join("billing.md");
        std::fs::write(
            &note_path,
            "# Billing\n\n#finance [[Retry Plan]]\nDo not index this private body phrase.",
        )
        .expect("write note");
        let store = LocalStore::in_memory().expect("store");
        let indexer = ObsidianIndexer::new(dir.path());

        let indexed = indexer.index_vault(&store).expect("index");

        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].note_path, "billing.md");
        assert_eq!(indexed[0].headings, vec!["Billing"]);
        let note_results = store.search(&SearchQuery::text("Retry")).expect("search");
        let body_results = store
            .search(&SearchQuery::text("private body phrase"))
            .expect("body search");

        assert_eq!(note_results[0].record_type, "note_metadata");
        assert_eq!(
            note_results[0].trust,
            SearchContentTrust::ExternalUntrustedData
        );
        assert!(body_results.is_empty());
    }
}
