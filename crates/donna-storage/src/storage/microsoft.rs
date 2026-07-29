use crate::storage::connection::{LocalStore, StorageError, now_seconds};
use crate::storage::types::{
    CalendarAttendee, CalendarEvent, NewCalendarEvent, NewOutlookMessage, NewTeamsMessage,
    OutlookMessage, TeamsMessage,
};
use rusqlite::types::Value;
use rusqlite::{Row, params, params_from_iter};

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
                is_cancelled, is_deleted, is_all_day
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
                is_deleted = excluded.is_deleted,
                is_all_day = excluded.is_all_day",
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
                input.is_all_day as i64,
            ],
        )?;

        let mut event = self.calendar_event_by_external_id(&input.external_id)?;
        self.replace_calendar_event_attendees(event.id, &input.attendees)?;
        event.attendees = input.attendees.clone();

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

    /// Replaces the full attendee list for a synced event — attendees have
    /// no stable per-row external id from Graph, so each sync just clears
    /// and re-inserts the current set rather than diffing.
    fn replace_calendar_event_attendees(
        &self,
        calendar_event_id: i64,
        attendees: &[CalendarAttendee],
    ) -> Result<(), StorageError> {
        self.connection.execute(
            "DELETE FROM calendar_event_attendees WHERE calendar_event_id = ?1",
            [calendar_event_id],
        )?;
        for attendee in attendees {
            self.connection.execute(
                "INSERT INTO calendar_event_attendees (calendar_event_id, name, email, is_optional)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(calendar_event_id, email) DO UPDATE SET
                    name = excluded.name,
                    is_optional = excluded.is_optional",
                params![
                    calendar_event_id,
                    &attendee.name,
                    &attendee.email,
                    attendee.is_optional as i64,
                ],
            )?;
        }
        Ok(())
    }

    pub fn calendar_event_by_external_id(
        &self,
        external_id: &str,
    ) -> Result<CalendarEvent, StorageError> {
        let mut event: CalendarEvent = self
            .connection
            .query_row(
                "SELECT id, external_id, subject, organizer_name, organizer_email,
                    starts_at, ends_at, original_timezone, show_as, synced_at,
                    etag, change_key, is_cancelled, is_deleted, is_all_day
                 FROM calendar_events
                 WHERE external_id = ?1",
                [external_id],
                calendar_event_from_row,
            )
            .map_err(StorageError::from)?;
        event.attendees = self.calendar_event_attendees(event.id)?;
        Ok(event)
    }

    /// Attendees for a single synced calendar event, in the order Microsoft
    /// Graph returned them.
    pub fn calendar_event_attendees(
        &self,
        calendar_event_id: i64,
    ) -> Result<Vec<CalendarAttendee>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT name, email, is_optional
             FROM calendar_event_attendees
             WHERE calendar_event_id = ?1
             ORDER BY id",
        )?;
        let attendees = statement
            .query_map([calendar_event_id], calendar_attendee_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(attendees)
    }

    fn with_attendees(
        &self,
        mut events: Vec<CalendarEvent>,
    ) -> Result<Vec<CalendarEvent>, StorageError> {
        for event in &mut events {
            event.attendees = self.calendar_event_attendees(event.id)?;
        }
        Ok(events)
    }

    /// Looks up a single synced event by its local id — the id a prior tool
    /// result surfaced to the model — so a follow-up question about "that
    /// meeting" can be answered by direct lookup instead of a fresh,
    /// potentially ambiguous text/date search.
    pub fn calendar_event_by_id(
        &self,
        id: i64,
    ) -> Result<Option<CalendarEvent>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, external_id, subject, organizer_name, organizer_email,
                starts_at, ends_at, original_timezone, show_as, synced_at,
                etag, change_key, is_cancelled, is_deleted, is_all_day
             FROM calendar_events
             WHERE id = ?1 AND is_deleted = 0 AND is_cancelled = 0",
        )?;
        let events = statement
            .query_map([id], calendar_event_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.with_attendees(events)?.into_iter().next())
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
                etag, change_key, is_cancelled, is_deleted, is_all_day
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
        self.with_attendees(events)
    }

    pub fn prune_outlook_messages_before(&self, cutoff_received_at: i64) -> Result<usize, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM outlook_messages
             WHERE received_at IS NOT NULL
                AND received_at < ?1",
        )?;
        let ids = statement
            .query_map([cutoff_received_at], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;

        for id in &ids {
            self.delete_search_record("outlook_message", *id)?;
        }

        self.connection.execute(
            "DELETE FROM outlook_messages
             WHERE received_at IS NOT NULL
                AND received_at < ?1",
            [cutoff_received_at],
        )?;
        Ok(ids.len())
    }

    pub fn prune_teams_messages_before(&self, cutoff_sent_at: i64) -> Result<usize, StorageError> {
        let pruned = self.connection.execute(
            "DELETE FROM search_index
             WHERE record_type = 'teams_message'
                AND record_id IN (
                    SELECT id FROM teams_messages
                    WHERE sent_at IS NOT NULL
                       AND sent_at < ?1
                )",
            [cutoff_sent_at],
        )?;

        self.connection.execute(
            "DELETE FROM teams_messages
             WHERE sent_at IS NOT NULL
                AND sent_at < ?1",
            [cutoff_sent_at],
        )?;
        Ok(pruned)
    }

    pub fn prune_calendar_events_outside_range(
        &self,
        starts_after_or_at: i64,
        ends_before_or_at: i64,
    ) -> Result<usize, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id FROM calendar_events
             WHERE ends_at < ?1
                OR starts_at > ?2",
        )?;
        let ids = statement
            .query_map(params![starts_after_or_at, ends_before_or_at], |row| {
                row.get::<_, i64>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for id in &ids {
            self.delete_search_record("calendar_event", *id)?;
        }

        self.connection.execute(
            "DELETE FROM calendar_events
             WHERE ends_at < ?1
                OR starts_at > ?2",
            params![starts_after_or_at, ends_before_or_at],
        )?;
        Ok(ids.len())
    }

    pub fn recent_teams_messages_from_sender(
        &self,
        sender_query: &str,
        sent_after_or_at: i64,
        limit: usize,
    ) -> Result<Vec<TeamsMessage>, StorageError> {
        let like = format!("%{}%", sender_query.trim().to_ascii_lowercase());
        let mut statement = self.connection.prepare(
            "SELECT id, external_id, chat_id, sender_name, sender_external_id,
                body, importance, web_url, sent_at, synced_at, etag, change_key,
                is_deleted
             FROM teams_messages
             WHERE is_deleted = 0
                AND sent_at IS NOT NULL
                AND sent_at >= ?1
                AND lower(coalesce(sender_name, '')) LIKE ?2
             ORDER BY sent_at DESC
             LIMIT ?3",
        )?;

        let messages = statement
            .query_map(
                params![sent_after_or_at, like, limit.clamp(1, 100) as i64],
                teams_message_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn calendar_events_in_range(
        &self,
        starts_before: i64,
        ends_after: i64,
        limit: usize,
    ) -> Result<Vec<CalendarEvent>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT id, external_id, subject, organizer_name, organizer_email,
                starts_at, ends_at, original_timezone, show_as, synced_at,
                etag, change_key, is_cancelled, is_deleted, is_all_day
             FROM calendar_events
             WHERE is_cancelled = 0
                AND is_deleted = 0
                AND starts_at IS NOT NULL
                AND ends_at IS NOT NULL
                AND starts_at < ?1
                AND ends_at > ?2
                AND lower(coalesce(show_as, 'busy')) IN ('busy', 'tentative', 'oof')
                AND is_all_day = 0
                AND NOT (subject IS NULL AND organizer_name IS NULL AND organizer_email IS NULL)
             ORDER BY starts_at ASC
             LIMIT ?3",
        )?;

        let events = statement
            .query_map(
                params![starts_before, ends_after, limit.clamp(1, 100) as i64],
                calendar_event_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        self.with_attendees(events)
    }

    pub fn list_outlook_messages(
        &self,
        received_after: Option<i64>,
        received_before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<OutlookMessage>, StorageError> {
        let mut sql = String::from(
            "SELECT id, external_id, folder_id, subject, sender_name, sender_email,
                body_preview, received_at, synced_at, etag, change_key, is_deleted
             FROM outlook_messages
             WHERE is_deleted = 0",
        );
        let mut values = Vec::new();

        if let Some(received_after) = received_after {
            sql.push_str(" AND received_at IS NOT NULL AND received_at >= ?");
            values.push(Value::Integer(received_after));
        }
        if let Some(received_before) = received_before {
            sql.push_str(" AND received_at IS NOT NULL AND received_at <= ?");
            values.push(Value::Integer(received_before));
        }
        sql.push_str(" ORDER BY coalesce(received_at, 0) DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let messages = statement
            .query_map(params_from_iter(values.iter()), outlook_message_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn search_outlook_messages(
        &self,
        title: Option<&str>,
        text: Option<&str>,
        person: Option<&str>,
        received_after: Option<i64>,
        received_before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<OutlookMessage>, StorageError> {
        let mut sql = String::from(
            "SELECT id, external_id, folder_id, subject, sender_name, sender_email,
                body_preview, received_at, synced_at, etag, change_key, is_deleted
             FROM outlook_messages
             WHERE is_deleted = 0",
        );
        let mut values = Vec::new();

        if let Some(title) = title.filter(|value| !value.trim().is_empty()) {
            sql.push_str(" AND lower(coalesce(subject, '')) LIKE ?");
            values.push(Value::Text(format!("%{}%", title.trim().to_ascii_lowercase())));
        }
        if let Some(text) = text.filter(|value| !value.trim().is_empty()) {
            let like = Value::Text(format!("%{}%", text.trim().to_ascii_lowercase()));
            sql.push_str(" AND (lower(coalesce(subject, '')) LIKE ? OR lower(coalesce(body_preview, '')) LIKE ?)");
            values.push(like.clone());
            values.push(like);
        }
        if let Some(person) = person.filter(|value| !value.trim().is_empty()) {
            let like = Value::Text(format!("%{}%", person.trim().to_ascii_lowercase()));
            sql.push_str(" AND (lower(coalesce(sender_name, '')) LIKE ? OR lower(coalesce(sender_email, '')) LIKE ?)");
            values.push(like.clone());
            values.push(like);
        }
        if let Some(received_after) = received_after {
            sql.push_str(" AND received_at IS NOT NULL AND received_at >= ?");
            values.push(Value::Integer(received_after));
        }
        if let Some(received_before) = received_before {
            sql.push_str(" AND received_at IS NOT NULL AND received_at <= ?");
            values.push(Value::Integer(received_before));
        }

        sql.push_str(" ORDER BY coalesce(received_at, 0) DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let messages = statement
            .query_map(params_from_iter(values.iter()), outlook_message_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn list_teams_messages(
        &self,
        sent_after: Option<i64>,
        sent_before: Option<i64>,
        conversation_like: Option<&str>,
        limit: usize,
    ) -> Result<Vec<TeamsMessage>, StorageError> {
        let mut sql = String::from(
            "SELECT id, external_id, chat_id, sender_name, sender_external_id,
                body, importance, web_url, sent_at, synced_at, etag, change_key,
                is_deleted
             FROM teams_messages
             WHERE is_deleted = 0",
        );
        let mut values = Vec::new();

        if let Some(sent_after) = sent_after {
            sql.push_str(" AND sent_at IS NOT NULL AND sent_at >= ?");
            values.push(Value::Integer(sent_after));
        }
        if let Some(sent_before) = sent_before {
            sql.push_str(" AND sent_at IS NOT NULL AND sent_at <= ?");
            values.push(Value::Integer(sent_before));
        }
        if let Some(conversation_like) = conversation_like.filter(|value| !value.trim().is_empty()) {
            sql.push_str(" AND lower(chat_id) LIKE ?");
            values.push(Value::Text(format!("%{}%", conversation_like.trim().to_ascii_lowercase())));
        }

        sql.push_str(" ORDER BY coalesce(sent_at, 0) DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let messages = statement
            .query_map(params_from_iter(values.iter()), teams_message_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn search_teams_messages(
        &self,
        text: Option<&str>,
        person: Option<&str>,
        sent_after: Option<i64>,
        sent_before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<TeamsMessage>, StorageError> {
        let mut sql = String::from(
            "SELECT id, external_id, chat_id, sender_name, sender_external_id,
                body, importance, web_url, sent_at, synced_at, etag, change_key,
                is_deleted
             FROM teams_messages
             WHERE is_deleted = 0",
        );
        let mut values = Vec::new();

        if let Some(text) = text.filter(|value| !value.trim().is_empty()) {
            sql.push_str(" AND lower(coalesce(body, '')) LIKE ?");
            values.push(Value::Text(format!("%{}%", text.trim().to_ascii_lowercase())));
        }
        if let Some(person) = person.filter(|value| !value.trim().is_empty()) {
            let like = Value::Text(format!("%{}%", person.trim().to_ascii_lowercase()));
            sql.push_str(" AND (lower(coalesce(sender_name, '')) LIKE ? OR lower(coalesce(sender_external_id, '')) LIKE ?)");
            values.push(like.clone());
            values.push(like);
        }
        if let Some(sent_after) = sent_after {
            sql.push_str(" AND sent_at IS NOT NULL AND sent_at >= ?");
            values.push(Value::Integer(sent_after));
        }
        if let Some(sent_before) = sent_before {
            sql.push_str(" AND sent_at IS NOT NULL AND sent_at <= ?");
            values.push(Value::Integer(sent_before));
        }

        sql.push_str(" ORDER BY coalesce(sent_at, 0) DESC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let messages = statement
            .query_map(params_from_iter(values.iter()), teams_message_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(messages)
    }

    pub fn list_teams_conversations(
        &self,
        channels_only: bool,
        limit: usize,
    ) -> Result<Vec<String>, StorageError> {
        let mut sql = String::from(
            "SELECT DISTINCT chat_id
             FROM teams_messages
             WHERE is_deleted = 0",
        );
        if channels_only {
            sql.push_str(" AND (chat_id LIKE '%/%' OR lower(chat_id) LIKE 'teams-channel%')");
        } else {
            sql.push_str(" AND chat_id NOT LIKE '%/%' AND lower(chat_id) NOT LIKE 'teams-channel%'");
        }
        sql.push_str(" ORDER BY chat_id LIMIT ?");

        let mut statement = self.connection.prepare(&sql)?;
        let conversations = statement
            .query_map([limit.clamp(1, 200) as i64], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    pub fn list_all_teams_channel_conversations(&self) -> Result<Vec<String>, StorageError> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT chat_id
             FROM teams_messages
             WHERE is_deleted = 0
               AND (chat_id LIKE '%/%' OR lower(chat_id) LIKE 'teams-channel%')
             ORDER BY chat_id",
        )?;

        let conversations = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    pub fn list_calendar_events(
        &self,
        starts_after: Option<i64>,
        starts_before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CalendarEvent>, StorageError> {
        let mut sql = String::from(
            "SELECT id, external_id, subject, organizer_name, organizer_email,
                starts_at, ends_at, original_timezone, show_as, synced_at,
                etag, change_key, is_cancelled, is_deleted, is_all_day
             FROM calendar_events
             WHERE is_deleted = 0 AND is_cancelled = 0
                AND NOT (subject IS NULL AND organizer_name IS NULL AND organizer_email IS NULL)",
        );
        let mut values = Vec::new();

        if let Some(starts_after) = starts_after {
            sql.push_str(" AND starts_at IS NOT NULL AND starts_at >= ?");
            values.push(Value::Integer(starts_after));
        }
        if let Some(starts_before) = starts_before {
            sql.push_str(" AND starts_at IS NOT NULL AND starts_at <= ?");
            values.push(Value::Integer(starts_before));
        }

        sql.push_str(" ORDER BY coalesce(starts_at, 0) ASC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let events = statement
            .query_map(params_from_iter(values.iter()), calendar_event_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        self.with_attendees(events)
    }

    pub fn search_calendar_events(
        &self,
        text: Option<&str>,
        people: Option<&str>,
        starts_after: Option<i64>,
        starts_before: Option<i64>,
        limit: usize,
    ) -> Result<Vec<CalendarEvent>, StorageError> {
        let mut sql = String::from(
            "SELECT id, external_id, subject, organizer_name, organizer_email,
                starts_at, ends_at, original_timezone, show_as, synced_at,
                etag, change_key, is_cancelled, is_deleted, is_all_day
             FROM calendar_events
             WHERE is_deleted = 0 AND is_cancelled = 0
                AND NOT (subject IS NULL AND organizer_name IS NULL AND organizer_email IS NULL)",
        );
        let mut values = Vec::new();

        // A local model unreliably splits a person's name between `text` and
        // `people` — sometimes putting the name in `text` alone. Rather than
        // relying on the model to pick the right field, both filters search
        // every place a name could live: the organizer and the full
        // attendee list, not just the subject line.
        if let Some(text) = text.filter(|value| !value.trim().is_empty()) {
            let like = Value::Text(format!("%{}%", text.trim().to_ascii_lowercase()));
            sql.push_str(
                " AND (lower(coalesce(subject, '')) LIKE ?
                    OR lower(coalesce(organizer_name, '')) LIKE ?
                    OR lower(coalesce(organizer_email, '')) LIKE ?
                    OR EXISTS (
                        SELECT 1 FROM calendar_event_attendees a
                        WHERE a.calendar_event_id = calendar_events.id
                          AND (lower(coalesce(a.name, '')) LIKE ? OR lower(coalesce(a.email, '')) LIKE ?)
                    ))",
            );
            for _ in 0..5 {
                values.push(like.clone());
            }
        }
        if let Some(people) = people.filter(|value| !value.trim().is_empty()) {
            let like = Value::Text(format!("%{}%", people.trim().to_ascii_lowercase()));
            sql.push_str(
                " AND (lower(coalesce(organizer_name, '')) LIKE ?
                    OR lower(coalesce(organizer_email, '')) LIKE ?
                    OR EXISTS (
                        SELECT 1 FROM calendar_event_attendees a
                        WHERE a.calendar_event_id = calendar_events.id
                          AND (lower(coalesce(a.name, '')) LIKE ? OR lower(coalesce(a.email, '')) LIKE ?)
                    ))",
            );
            for _ in 0..4 {
                values.push(like.clone());
            }
        }
        if let Some(starts_after) = starts_after {
            sql.push_str(" AND starts_at IS NOT NULL AND starts_at >= ?");
            values.push(Value::Integer(starts_after));
        }
        if let Some(starts_before) = starts_before {
            sql.push_str(" AND starts_at IS NOT NULL AND starts_at <= ?");
            values.push(Value::Integer(starts_before));
        }

        sql.push_str(" ORDER BY coalesce(starts_at, 0) ASC LIMIT ?");
        values.push(Value::Integer(limit.clamp(1, 200) as i64));

        let mut statement = self.connection.prepare(&sql)?;
        let events = statement
            .query_map(params_from_iter(values.iter()), calendar_event_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        self.with_attendees(events)
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
    let is_all_day: i64 = row.get(14)?;
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
        is_all_day: is_all_day != 0,
        attendees: Vec::new(),
    })
}

fn calendar_attendee_from_row(row: &Row<'_>) -> rusqlite::Result<CalendarAttendee> {
    let is_optional: i64 = row.get(2)?;
    Ok(CalendarAttendee {
        name: row.get(0)?,
        email: row.get(1)?,
        is_optional: is_optional != 0,
    })
}
