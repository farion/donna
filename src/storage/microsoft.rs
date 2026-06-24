use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::types::{
    CalendarEvent, NewCalendarEvent, NewOutlookMessage, NewTeamsMessage, OutlookMessage,
    TeamsMessage,
};
use rusqlite::{Row, params};

impl LocalStore {
    pub fn upsert_outlook_message(
        &self,
        input: &NewOutlookMessage,
    ) -> Result<OutlookMessage, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO outlook_messages (
                external_id, folder_id, subject, sender_name, sender_email,
                body_preview, received_at, synced_at, etag, change_key, is_deleted
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(external_id) DO UPDATE SET
                folder_id = excluded.folder_id,
                subject = excluded.subject,
                sender_name = excluded.sender_name,
                sender_email = excluded.sender_email,
                body_preview = excluded.body_preview,
                received_at = excluded.received_at,
                synced_at = excluded.synced_at,
                etag = excluded.etag,
                change_key = excluded.change_key,
                is_deleted = excluded.is_deleted",
            params![
                &input.external_id,
                &input.folder_id,
                &input.subject,
                &input.sender_name,
                &input.sender_email,
                &input.body_preview,
                input.received_at,
                now,
                &input.etag,
                &input.change_key,
                input.is_deleted as i64,
            ],
        )?;

        let message = self.outlook_message_by_external_id(&input.external_id)?;
        if message.is_deleted {
            self.delete_search_record("outlook_message", message.id)?;
        } else {
            self.replace_search_record(
                "outlook_message",
                message.id,
                message.subject.as_deref().unwrap_or(""),
                message.body_preview.as_deref().unwrap_or(""),
                "outlook",
            )?;
        }
        Ok(message)
    }

    pub fn outlook_message_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<OutlookMessage, StorageError> {
        self.connection
            .query_row(
                "SELECT id, external_id, folder_id, subject, sender_name, sender_email,
                    body_preview, received_at, synced_at, etag, change_key, is_deleted
                 FROM outlook_messages
                 WHERE external_id = ?1",
                [external_id],
                outlook_message_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn upsert_teams_message(
        &self,
        input: &NewTeamsMessage,
    ) -> Result<TeamsMessage, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO teams_messages (
                external_id, chat_id, sender_name, sender_external_id, body,
                importance, web_url, sent_at, synced_at, etag, change_key, is_deleted
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(external_id) DO UPDATE SET
                chat_id = excluded.chat_id,
                sender_name = excluded.sender_name,
                sender_external_id = excluded.sender_external_id,
                body = excluded.body,
                importance = excluded.importance,
                web_url = excluded.web_url,
                sent_at = excluded.sent_at,
                synced_at = excluded.synced_at,
                etag = excluded.etag,
                change_key = excluded.change_key,
                is_deleted = excluded.is_deleted",
            params![
                &input.external_id,
                &input.chat_id,
                &input.sender_name,
                &input.sender_external_id,
                &input.body,
                &input.importance,
                &input.web_url,
                input.sent_at,
                now,
                &input.etag,
                &input.change_key,
                input.is_deleted as i64,
            ],
        )?;

        let message = self.teams_message_by_external_id(&input.external_id)?;
        if message.is_deleted {
            self.delete_search_record("teams_message", message.id)?;
        } else {
            self.replace_search_record(
                "teams_message",
                message.id,
                message.sender_name.as_deref().unwrap_or(""),
                &message.body,
                "teams",
            )?;
        }
        Ok(message)
    }

    pub fn teams_message_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<TeamsMessage, StorageError> {
        self.connection
            .query_row(
                "SELECT id, external_id, chat_id, sender_name, sender_external_id,
                    body, importance, web_url, sent_at, synced_at, etag, change_key,
                    is_deleted
                 FROM teams_messages
                 WHERE external_id = ?1",
                [external_id],
                teams_message_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn upsert_calendar_event(
        &self,
        input: &NewCalendarEvent,
    ) -> Result<CalendarEvent, StorageError> {
        let now = now_seconds()?;
        self.connection.execute(
            "INSERT INTO calendar_events (
                external_id, subject, organizer_name, organizer_email, starts_at,
                ends_at, original_timezone, show_as, synced_at, etag, change_key,
                is_cancelled, is_deleted
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
             ON CONFLICT(external_id) DO UPDATE SET
                subject = excluded.subject,
                organizer_name = excluded.organizer_name,
                organizer_email = excluded.organizer_email,
                starts_at = excluded.starts_at,
                ends_at = excluded.ends_at,
                original_timezone = excluded.original_timezone,
                show_as = excluded.show_as,
                synced_at = excluded.synced_at,
                etag = excluded.etag,
                change_key = excluded.change_key,
                is_cancelled = excluded.is_cancelled,
                is_deleted = excluded.is_deleted",
            params![
                &input.external_id,
                &input.subject,
                &input.organizer_name,
                &input.organizer_email,
                input.starts_at,
                input.ends_at,
                &input.original_timezone,
                &input.show_as,
                now,
                &input.etag,
                &input.change_key,
                input.is_cancelled as i64,
                input.is_deleted as i64,
            ],
        )?;

        let event = self.calendar_event_by_external_id(&input.external_id)?;
        if event.is_deleted || event.is_cancelled {
            self.delete_search_record("calendar_event", event.id)?;
        } else {
            self.replace_search_record(
                "calendar_event",
                event.id,
                event.subject.as_deref().unwrap_or(""),
                event.organizer_name.as_deref().unwrap_or(""),
                "calendar",
            )?;
        }
        Ok(event)
    }

    pub fn calendar_event_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<CalendarEvent, StorageError> {
        self.connection
            .query_row(
                "SELECT id, external_id, subject, organizer_name, organizer_email,
                    starts_at, ends_at, original_timezone, show_as, synced_at,
                    etag, change_key, is_cancelled, is_deleted
                 FROM calendar_events
                 WHERE external_id = ?1",
                [external_id],
                calendar_event_from_row,
            )
            .map_err(StorageError::from)
    }

    pub fn calendar_collisions(
        &self,
        starts_at: i64,
        ends_at: i64,
    ) -> Result<Vec<CalendarEvent>, StorageError> {
        if starts_at >= ends_at {
            return Ok(Vec::new());
        }

        let mut statement = self.connection.prepare(
            "SELECT id, external_id, subject, organizer_name, organizer_email,
                starts_at, ends_at, original_timezone, show_as, synced_at,
                etag, change_key, is_cancelled, is_deleted
             FROM calendar_events
             WHERE is_cancelled = 0
                AND is_deleted = 0
                AND starts_at IS NOT NULL
                AND ends_at IS NOT NULL
                AND starts_at < ?2
                AND ends_at > ?1
                AND lower(coalesce(show_as, 'busy')) IN ('busy', 'tentative', 'oof')
             ORDER BY starts_at",
        )?;

        let events = statement
            .query_map(params![starts_at, ends_at], calendar_event_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }
}

fn outlook_message_from_row(row: &Row<'_>) -> rusqlite::Result<OutlookMessage> {
    let is_deleted: i64 = row.get(11)?;
    Ok(OutlookMessage {
        id: row.get(0)?,
        external_id: row.get(1)?,
        folder_id: row.get(2)?,
        subject: row.get(3)?,
        sender_name: row.get(4)?,
        sender_email: row.get(5)?,
        body_preview: row.get(6)?,
        received_at: row.get(7)?,
        synced_at: row.get(8)?,
        etag: row.get(9)?,
        change_key: row.get(10)?,
        is_deleted: is_deleted != 0,
    })
}

fn teams_message_from_row(row: &Row<'_>) -> rusqlite::Result<TeamsMessage> {
    let is_deleted: i64 = row.get(12)?;
    Ok(TeamsMessage {
        id: row.get(0)?,
        external_id: row.get(1)?,
        chat_id: row.get(2)?,
        sender_name: row.get(3)?,
        sender_external_id: row.get(4)?,
        body: row.get(5)?,
        importance: row.get(6)?,
        web_url: row.get(7)?,
        sent_at: row.get(8)?,
        synced_at: row.get(9)?,
        etag: row.get(10)?,
        change_key: row.get(11)?,
        is_deleted: is_deleted != 0,
    })
}

fn calendar_event_from_row(row: &Row<'_>) -> rusqlite::Result<CalendarEvent> {
    let is_cancelled: i64 = row.get(12)?;
    let is_deleted: i64 = row.get(13)?;
    Ok(CalendarEvent {
        id: row.get(0)?,
        external_id: row.get(1)?,
        subject: row.get(2)?,
        organizer_name: row.get(3)?,
        organizer_email: row.get(4)?,
        starts_at: row.get(5)?,
        ends_at: row.get(6)?,
        original_timezone: row.get(7)?,
        show_as: row.get(8)?,
        synced_at: row.get(9)?,
        etag: row.get(10)?,
        change_key: row.get(11)?,
        is_cancelled: is_cancelled != 0,
        is_deleted: is_deleted != 0,
    })
}
