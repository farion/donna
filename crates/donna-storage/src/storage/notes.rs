use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::types::{NewNoteMetadata, NoteMetadata};
use rusqlite::{OptionalExtension, Row, params};

impl LocalStore {
    pub fn upsert_note_metadata(
        &self,
        input: &NewNoteMetadata,
    ) -> Result<NoteMetadata, StorageError> {
        let now = now_seconds()?;
        let headings = encode_list(&input.headings)?;
        let tags = encode_list(&input.tags)?;
        let links = encode_list(&input.links)?;

        self.connection.execute(
            "INSERT INTO notes_metadata (
                vault_path, note_path, title, headings, tags, links, modified_at, indexed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(vault_path, note_path) DO UPDATE SET
                title = excluded.title,
                headings = excluded.headings,
                tags = excluded.tags,
                links = excluded.links,
                modified_at = excluded.modified_at,
                indexed_at = excluded.indexed_at",
            params![
                &input.vault_path,
                &input.note_path,
                &input.title,
                &headings,
                &tags,
                &links,
                input.modified_at,
                now
            ],
        )?;

        let note = self
            .note_metadata(&input.vault_path, &input.note_path)?
            .ok_or(rusqlite::Error::QueryReturnedNoRows)?;
        self.replace_search_record(
            "note_metadata",
            note.id,
            note.title.as_deref().unwrap_or(&note.note_path),
            &note.search_body(),
            "obsidian",
        )?;
        Ok(note)
    }

    pub fn note_metadata(
        &self,
        vault_path: &str,
        note_path: &str,
    ) -> Result<Option<NoteMetadata>, StorageError> {
        self.connection
            .query_row(
                "SELECT id, vault_path, note_path, title, headings, tags, links,
                    modified_at, indexed_at
                 FROM notes_metadata
                 WHERE vault_path = ?1 AND note_path = ?2",
                params![vault_path, note_path],
                note_metadata_from_row,
            )
            .optional()
            .map_err(StorageError::from)
    }

    pub fn list_note_metadata(&self) -> Result<Vec<NoteMetadata>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, vault_path, note_path, title, headings, tags, links,
                modified_at, indexed_at
             FROM notes_metadata
             ORDER BY vault_path, note_path",
        )?;
        let notes = statement
            .query_map([], note_metadata_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(notes)
    }

    pub fn remove_note_metadata(
        &self,
        vault_path: &str,
        note_path: &str,
    ) -> Result<(), StorageError> {
        if let Some(note) = self.note_metadata(vault_path, note_path)? {
            self.connection
                .execute("DELETE FROM notes_metadata WHERE id = ?1", params![note.id])?;
            self.delete_search_record("note_metadata", note.id)?;
        }
        Ok(())
    }
}

impl NoteMetadata {
    fn search_body(&self) -> String {
        self.headings
            .iter()
            .chain(self.tags.iter())
            .chain(self.links.iter())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn note_metadata_from_row(row: &Row<'_>) -> rusqlite::Result<NoteMetadata> {
    Ok(NoteMetadata {
        id: row.get(0)?,
        vault_path: row.get(1)?,
        note_path: row.get(2)?,
        title: row.get(3)?,
        headings: decode_list(row.get::<_, String>(4)?.as_str())?,
        tags: decode_list(row.get::<_, String>(5)?.as_str())?,
        links: decode_list(row.get::<_, String>(6)?.as_str())?,
        modified_at: row.get(7)?,
        indexed_at: row.get(8)?,
    })
}

fn encode_list(values: &[String]) -> Result<String, StorageError> {
    serde_json::to_string(values).map_err(|error| {
        StorageError::Sqlite(rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
    })
}

fn decode_list(value: &str) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })
}
