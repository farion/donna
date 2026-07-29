use crate::microsoft::auth::{MicrosoftTokenSet, percent_encode};
use crate::microsoft::calendar::CALENDAR_SOURCE;
use crate::microsoft::error::GraphError;
use crate::microsoft::outlook::OUTLOOK_MAIL_SOURCE;
use crate::microsoft::teams::{TEAMS_CHANNEL_SOURCE, TEAMS_CHAT_SOURCE};
use donna_config::MicrosoftConfig;
use donna_storage::{
    CalendarAttendee, LocalStore, NewCalendarEvent, NewOutlookMessage, NewSyncState,
    NewTeamsMessage,
};
use reqwest::blocking::Client;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DAY_SECONDS: i64 = 86_400;
const LOOKBACK_DAYS: i64 = 90;
const LOOKAHEAD_DAYS: i64 = 90;
const TEAMS_CHANNEL_CACHE_SECONDS: i64 = 21_600;
const TEAMS_CHANNEL_CACHE_KEY: &str = "microsoft.teams.channels.cache";
const TEAMS_CHANNEL_CACHE_LAST_AT_KEY: &str = "microsoft.teams.channels.cache.last_at";
const TEAMS_CHAT_SCAN_WATERMARK_KEY: &str = "microsoft.teams.chat.activity_watermark.v2";
const TEAMS_CHAT_CACHE_SECONDS: i64 = 600;
const TEAMS_CHAT_CACHE_KEY: &str = "microsoft.teams.chats.cache.v1";
const TEAMS_CHAT_CACHE_LAST_AT_KEY: &str = "microsoft.teams.chats.cache.last_at.v1";
const OUTLOOK_PROGRESS_KEY: &str = "microsoft.sync.progress.outlook";
const TEAMS_PROGRESS_KEY: &str = "microsoft.sync.progress.teams";
const CALENDAR_PROGRESS_KEY: &str = "microsoft.sync.progress.calendar";

pub struct GraphSyncClient {
    http: Client,
    access_token: String,
    teams_activity_window_days: u32,
}

impl GraphSyncClient {
    pub fn new(tokens: &MicrosoftTokenSet, config: &MicrosoftConfig) -> Self {
        Self {
            http: shared_http_client().clone(),
            access_token: tokens.access_token.clone(),
            teams_activity_window_days: config.teams_activity_window_days,
        }
    }

    pub fn sync_all(&self, store: &LocalStore) -> Result<(), GraphError> {
        let mut first_error = None;
        let mut success_count = 0usize;

        for (source, action) in [
            (OUTLOOK_MAIL_SOURCE, Self::sync_outlook as fn(&Self, &LocalStore) -> Result<(), GraphError>),
            (TEAMS_CHAT_SOURCE, Self::sync_teams_chat),
            (TEAMS_CHANNEL_SOURCE, Self::sync_teams_channel),
            (CALENDAR_SOURCE, Self::sync_calendar),
        ] {
            let started = Instant::now();
            match action(self, store) {
                Ok(()) => {
                    success_count += 1;
                    eprintln!(
                        "donna microsoft sync: source done (source={source}, elapsed_ms={})",
                        started.elapsed().as_millis()
                    );
                }
                Err(error) => {
                    let _ = mark_stale(store, source, &error.sync_error_message());
                    eprintln!(
                        "donna microsoft sync: source failed (source={source}, elapsed_ms={}): {error}",
                        started.elapsed().as_millis()
                    );
                    if first_error.is_none() {
                        first_error = Some(error);
                    }
                }
            }
        }

        if success_count > 0 {
            if first_error.is_some() {
                eprintln!(
                    "donna microsoft sync: completed with partial failures (successful_sources={success_count})"
                );
            }
            return Ok(());
        }

        match first_error {
            Some(error) => Err(error),
            None => Err(GraphError::Auth("no Microsoft sync source was executed".to_owned())),
        }
    }

    fn sync_outlook(&self, store: &LocalStore) -> Result<(), GraphError> {
        eprintln!("donna microsoft sync: outlook begin");
        let _ = store.set_runtime_state(OUTLOOK_PROGRESS_KEY, "0");
        let previous = store.sync_state(OUTLOOK_MAIL_SOURCE)?;
        let initial_url = previous
            .as_ref()
            .and_then(|state| state.cursor.clone().or_else(|| state.delta_link.clone()))
            .unwrap_or_else(|| {
                "https://graph.microsoft.com/v1.0/me/mailFolders/inbox/messages/delta?$top=100&$select=id,parentFolderId,subject,sender,bodyPreview,receivedDateTime,changeKey".to_owned()
            });
        let message_count = self.process_pages::<GraphMail, _>(
            store,
            OUTLOOK_MAIL_SOURCE,
            "outlook.mail",
            initial_url,
            None,
            |item| {
                store.upsert_outlook_message(&NewOutlookMessage {
                    external_id: item.id,
                    folder_id: item.parent_folder_id,
                    subject: item.subject,
                    sender_name: item.sender.as_ref().and_then(|sender| {
                        sender.email_address.as_ref().and_then(|e| e.name.clone())
                    }),
                    sender_email: item.sender.as_ref().and_then(|sender| {
                        sender.email_address.as_ref().and_then(|e| e.address.clone())
                    }),
                    body_preview: item.body_preview,
                    received_at: item.received_date_time.as_deref().and_then(parse_graph_time),
                    etag: item.etag,
                    change_key: item.change_key,
                    is_deleted: false,
                })?;
                Ok(())
            },
        )?;

        let now = now_seconds()?;
        let cutoff = now - (LOOKBACK_DAYS * DAY_SECONDS);
        let pruned = if should_prune(store, "microsoft.prune.outlook.last_at", now)? {
            let pruned = store.prune_outlook_messages_before(cutoff)?;
            store.set_runtime_state("microsoft.prune.outlook.last_at", &now.to_string())?;
            pruned
        } else {
            0
        };
        eprintln!(
            "donna microsoft sync: outlook done (fetched={message_count}, pruned={pruned})"
        );
        let _ = store.set_runtime_state(OUTLOOK_PROGRESS_KEY, "100");
        Ok(())
    }

    fn sync_teams_chat(&self, store: &LocalStore) -> Result<(), GraphError> {
        eprintln!("donna microsoft sync: teams chat begin");
        let _ = store.set_runtime_state(TEAMS_PROGRESS_KEY, "0");
        let mut chat_index = 0usize;
        let mut message_count = 0usize;
        let activity_cutoff = self.teams_activity_cutoff()?;
        let metadata_started = Instant::now();
        let scan_cutoff = self.chat_scan_cutoff(store, activity_cutoff)?;
        let (chats, latest_chat_activity_at) =
            self.collect_cached_or_live_recent_chats(store, activity_cutoff, scan_cutoff)?;
        let chats = self.filter_changed_chats(store, chats)?;
        let total_chats = chats.len();
        eprintln!(
            "donna microsoft sync: teams chat scope (chats_to_scan={total_chats}, scan_cutoff={}, metadata_elapsed_ms={})",
            scan_cutoff.map(|value| value.to_string()).unwrap_or_else(|| "none".to_owned()),
            metadata_started.elapsed().as_millis()
        );

        for chat in chats {
            chat_index += 1;
            let progress = percent_progress(chat_index, total_chats);
            let chat_started = Instant::now();
            let activity_at = chat.activity_at();
            let activity_state_key = format!("microsoft.teams.chat.activity_at.{}", chat.id);
            let last_synced_activity_at = activity_at.and_then(|_| {
                store
                    .runtime_state(&activity_state_key)
                    .ok()
                    .flatten()
                    .and_then(|value| value.parse::<i64>().ok())
            });
            eprintln!(
                "donna microsoft sync: teams chat stream begin (index={chat_index}/{total_chats}, progress={progress}%, chat_id={})",
                chat.id
            );
            let message_cutoff = incremental_message_cutoff(activity_cutoff, last_synced_activity_at);
            let default_url = format!(
                "https://graph.microsoft.com/v1.0/chats/{}/messages?$top=50&$orderby=lastModifiedDateTime desc&$filter={}",
                chat.id,
                graph_time_gt_filter("lastModifiedDateTime", message_cutoff)
            );
            let url = default_url.clone();
            let fetched = self.process_pages::<GraphTeamsMessage, _>(
                store,
                TEAMS_CHAT_SOURCE,
                &chat.id,
                url,
                None,
                |item| {
                    store.upsert_teams_message(&teams_message_from_graph(item, "teams-chat"))?;
                    Ok(())
                },
            )?;
            message_count += fetched;
            eprintln!(
                "donna microsoft sync: teams chat stream done (index={chat_index}/{total_chats}, progress={progress}%, chat_id={}, fetched={}, elapsed_ms={})",
                chat.id,
                fetched,
                chat_started.elapsed().as_millis()
            );
            if let Some(activity_at) = activity_at {
                store.set_runtime_state(&activity_state_key, &activity_at.to_string())?;
            }
            let _ = store.set_runtime_state(
                TEAMS_PROGRESS_KEY,
                &percent_progress(chat_index, total_chats.max(1)).to_string(),
            );
        }

        let now = now_seconds()?;
        let cutoff = now - (LOOKBACK_DAYS * DAY_SECONDS);
        let pruned = if should_prune(store, "microsoft.prune.teams.last_at", now)? {
            let pruned = store.prune_teams_messages_before(cutoff)?;
            store.set_runtime_state("microsoft.prune.teams.last_at", &now.to_string())?;
            pruned
        } else {
            0
        };
        eprintln!(
            "donna microsoft sync: teams chat done (chats={chat_index}, fetched={message_count}, pruned={pruned})"
        );
        let _ = store.set_runtime_state(TEAMS_PROGRESS_KEY, "100");
        if let Some(activity_at) = latest_chat_activity_at {
            store.set_runtime_state(TEAMS_CHAT_SCAN_WATERMARK_KEY, &activity_at.to_string())?;
        }
        Ok(())
    }

    fn sync_teams_channel(&self, store: &LocalStore) -> Result<(), GraphError> {
        eprintln!("donna microsoft sync: teams channel begin");
        let _ = store.set_runtime_state(TEAMS_PROGRESS_KEY, "0");
        let cutoff = self.teams_activity_cutoff()?;
        let metadata_started = Instant::now();
        let channels = self.collect_cached_or_fallback_team_channels(store)?;
        let total_channels = channels.len();
        let mut channel_index = 0usize;
        let mut message_count = 0usize;
        eprintln!(
            "donna microsoft sync: teams channel scope (channels_to_scan={total_channels}, metadata_elapsed_ms={})",
            metadata_started.elapsed().as_millis()
        );

        for channel in channels {
            channel_index += 1;
            let progress = percent_progress(channel_index, total_channels);
            let channel_started = Instant::now();
            let fetched = self.sync_channel_messages_for_channel(store, &channel, cutoff)?;
            message_count += fetched;
            eprintln!(
                "donna microsoft sync: teams channel progress (index={channel_index}/{total_channels}, progress={progress}%, fetched={fetched}, team_id={}, channel_id={}, elapsed_ms={})",
                channel.team_id,
                channel.channel_id,
                channel_started.elapsed().as_millis()
            );
            let _ = store.set_runtime_state(TEAMS_PROGRESS_KEY, &progress.to_string());
        }

        store.upsert_sync_state(&NewSyncState {
            source: TEAMS_CHANNEL_SOURCE.to_owned(),
            cursor: None,
            delta_link: None,
            last_sync_at: Some(now_seconds()?),
            last_error: None,
            is_stale: false,
        })?;

        let now = now_seconds()?;
        let cutoff = now - (LOOKBACK_DAYS * DAY_SECONDS);
        let pruned = if should_prune(store, "microsoft.prune.teams.last_at", now)? {
            let pruned = store.prune_teams_messages_before(cutoff)?;
            store.set_runtime_state("microsoft.prune.teams.last_at", &now.to_string())?;
            pruned
        } else {
            0
        };
        eprintln!(
            "donna microsoft sync: teams channel done (fetched={message_count}, pruned={pruned})"
        );
        let _ = store.set_runtime_state(TEAMS_PROGRESS_KEY, "100");
        Ok(())
    }

    fn sync_calendar(&self, store: &LocalStore) -> Result<(), GraphError> {
        eprintln!("donna microsoft sync: calendar begin");
        let _ = store.set_runtime_state(CALENDAR_PROGRESS_KEY, "0");
        let previous = store.sync_state(CALENDAR_SOURCE)?;
        let now = now_seconds()?;
        let start = now - (LOOKBACK_DAYS * DAY_SECONDS);
        let end = now + (LOOKAHEAD_DAYS * DAY_SECONDS);
        let initial_url = previous
            .as_ref()
            .and_then(|state| state.cursor.clone().or_else(|| state.delta_link.clone()))
            .unwrap_or_else(|| {
                format!(
                    "https://graph.microsoft.com/v1.0/me/calendarView/delta?startDateTime={}&endDateTime={}&$select=id,subject,organizer,start,end,originalStartTimeZone,showAs,isCancelled,changeKey,attendees,isAllDay",
                    format_graph_time(start),
                    format_graph_time(end)
                )
            });

        let mut backfill_attempted = 0usize;
        let mut backfill_recovered = 0usize;

        let event_count = self.process_pages::<GraphCalendarEvent, _>(
            store,
            CALENDAR_SOURCE,
            "calendar",
            initial_url,
            None,
            |item| {
                let (subject, organizer) = if item.subject.is_none() {
                    backfill_attempted += 1;
                    let detail = self.backfill_calendar_event_detail(&item.id);
                    if detail.as_ref().is_some_and(|detail| detail.subject.is_some()) {
                        backfill_recovered += 1;
                    }
                    detail
                        .map(|detail| (detail.subject, detail.organizer))
                        .unwrap_or((None, item.organizer.clone()))
                } else {
                    (item.subject, item.organizer.clone())
                };

                store.upsert_calendar_event(&NewCalendarEvent {
                    external_id: item.id,
                    subject,
                    organizer_name: organizer.as_ref().and_then(|organizer| {
                        organizer.email_address.as_ref().and_then(|e| e.name.clone())
                    }),
                    organizer_email: organizer.as_ref().and_then(|organizer| {
                        organizer.email_address.as_ref().and_then(|e| e.address.clone())
                    }),
                    starts_at: item
                        .start
                        .as_ref()
                        .and_then(|start| start.date_time.as_deref())
                        .and_then(parse_graph_time),
                    ends_at: item
                        .end
                        .as_ref()
                        .and_then(|end| end.date_time.as_deref())
                        .and_then(parse_graph_time),
                    original_timezone: item.original_start_time_zone,
                    show_as: item.show_as,
                    etag: item.etag,
                    change_key: item.change_key,
                    is_cancelled: item.is_cancelled.unwrap_or(false),
                    is_deleted: false,
                    is_all_day: item.is_all_day.unwrap_or(false),
                    attendees: item
                        .attendees
                        .into_iter()
                        .filter_map(GraphAttendee::into_calendar_attendee)
                        .collect::<Vec<CalendarAttendee>>(),
                })?;
                Ok(())
            },
        )?;

        let pruned = if should_prune(store, "microsoft.prune.calendar.last_at", now)? {
            let pruned = store.prune_calendar_events_outside_range(start, end)?;
            store.set_runtime_state("microsoft.prune.calendar.last_at", &now.to_string())?;
            pruned
        } else {
            0
        };
        eprintln!(
            "donna microsoft sync: calendar done (fetched={event_count}, pruned={pruned}, \
             subject_backfill_attempts={backfill_attempted}, subject_backfill_recovered={backfill_recovered})"
        );
        let _ = store.set_runtime_state(CALENDAR_PROGRESS_KEY, "100");
        Ok(())
    }

    /// Microsoft Graph's `calendarView/delta` can omit `subject` (and other
    /// properties) for recurring-event occurrences even when `$select`
    /// explicitly requests it — a known Graph limitation, not a client bug.
    /// Falling back to a direct per-event GET reliably returns full
    /// properties for that same occurrence id.
    fn backfill_calendar_event_detail(&self, event_id: &str) -> Option<GraphCalendarEventDetail> {
        let url = format!(
            "https://graph.microsoft.com/v1.0/me/events/{}?$select=subject,organizer",
            percent_encode(event_id)
        );
        match self.get::<GraphCalendarEventDetail>(&url) {
            Ok(detail) if detail.subject.is_none() => {
                eprintln!(
                    "donna microsoft sync: calendar event subject backfill for {event_id} \
                     succeeded but Graph returned no subject (event genuinely has none, or is \
                     restricted by sensitivity)"
                );
                Some(detail)
            }
            Ok(detail) => Some(detail),
            Err(error) => {
                eprintln!(
                    "donna microsoft sync: calendar event subject backfill failed for {event_id}: {error}"
                );
                None
            }
        }
    }

    fn teams_activity_cutoff(&self) -> Result<i64, GraphError> {
        if self.teams_activity_window_days == 0 {
            return Ok(0);
        }

        Ok(now_seconds()? - (self.teams_activity_window_days as i64 * DAY_SECONDS))
    }

    fn chat_scan_cutoff(
        &self,
        store: &LocalStore,
        activity_cutoff: i64,
    ) -> Result<Option<i64>, GraphError> {
        let watermark = store
            .runtime_state(TEAMS_CHAT_SCAN_WATERMARK_KEY)?
            .and_then(|value| value.parse::<i64>().ok());

        Ok(match watermark {
            Some(watermark) if watermark > activity_cutoff => Some(watermark),
            _ => None,
        })
    }

    fn collect_recent_chats(
        &self,
        activity_cutoff: i64,
        scan_cutoff: Option<i64>,
    ) -> Result<(Vec<GraphChat>, Option<i64>), GraphError> {
        let mut url = "https://graph.microsoft.com/v1.0/me/chats?$top=50&$orderby=lastMessagePreview/createdDateTime%20desc&$expand=lastMessagePreview".to_owned();
        let mut chats = Vec::new();
        let mut latest_chat_activity_at = None;
        let mut pages = 0usize;
        let started = Instant::now();

        loop {
            let page_started = Instant::now();
            let page: GraphPage<GraphChat> = self.get(&url)?;
            pages += 1;
            let page_records = page.value.len();
            let mut reached_cutoff = false;
            for chat in page.value {
                let activity_at = chat.activity_at();
                if latest_chat_activity_at.is_none() {
                    latest_chat_activity_at = activity_at;
                }
                if scan_cutoff.is_some_and(|cutoff| activity_at.is_some_and(|timestamp| timestamp < cutoff)) {
                    reached_cutoff = true;
                    break;
                }
                if chat.has_recent_activity(activity_cutoff) {
                    chats.push(chat);
                }
            }
            eprintln!(
                "donna microsoft sync: teams chat metadata progress (pages={pages}, page_records={page_records}, recent_candidates={}, page_elapsed_ms={}, total_elapsed_ms={})",
                chats.len(),
                page_started.elapsed().as_millis(),
                started.elapsed().as_millis()
            );
            if reached_cutoff {
                break;
            }
            match page.next_link {
                Some(next_link) => url = next_link,
                None => break,
            }
        }

        Ok((chats, latest_chat_activity_at))
    }

    fn collect_cached_or_live_recent_chats(
        &self,
        store: &LocalStore,
        activity_cutoff: i64,
        scan_cutoff: Option<i64>,
    ) -> Result<(Vec<GraphChat>, Option<i64>), GraphError> {
        let now = now_seconds()?;
        if let Some((cached, latest)) = self.load_cached_recent_chats(store, now)? {
            return Ok((cached, latest));
        }

        let (chats, latest) = self.collect_recent_chats(activity_cutoff, scan_cutoff)?;
        self.store_cached_recent_chats(store, &chats, now)?;
        Ok((chats, latest))
    }

    fn load_cached_recent_chats(
        &self,
        store: &LocalStore,
        now: i64,
    ) -> Result<Option<(Vec<GraphChat>, Option<i64>)>, GraphError> {
        let Some(last_at) = store.runtime_state(TEAMS_CHAT_CACHE_LAST_AT_KEY)? else {
            return Ok(None);
        };
        let Ok(last_at) = last_at.parse::<i64>() else {
            return Ok(None);
        };
        if now - last_at >= TEAMS_CHAT_CACHE_SECONDS {
            return Ok(None);
        }

        let Some(raw) = store.runtime_state(TEAMS_CHAT_CACHE_KEY)? else {
            return Ok(None);
        };
        let chats = raw
            .lines()
            .filter_map(|line| {
                let (id, activity_at) = line.split_once('\t')?;
                Some(GraphChat {
                    id: id.to_owned(),
                    last_message_preview: None,
                    last_updated_date_time: Some(activity_at.to_owned()),
                })
            })
            .collect::<Vec<_>>();
        if chats.is_empty() {
            return Ok(None);
        }

        let latest = chats.first().and_then(GraphChat::activity_at);
        eprintln!(
            "donna microsoft sync: teams chat cache hit (chats={}, age_seconds={})",
            chats.len(),
            now - last_at
        );
        Ok(Some((chats, latest)))
    }

    fn store_cached_recent_chats(
        &self,
        store: &LocalStore,
        chats: &[GraphChat],
        now: i64,
    ) -> Result<(), GraphError> {
        let raw = chats
            .iter()
            .filter_map(|chat| {
                chat.activity_at()
                    .map(|activity_at| format!("{}\t{}", chat.id, format_graph_time(activity_at)))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if raw.is_empty() {
            return Ok(());
        }
        store.set_runtime_state(TEAMS_CHAT_CACHE_KEY, &raw)?;
        store.set_runtime_state(TEAMS_CHAT_CACHE_LAST_AT_KEY, &now.to_string())?;
        eprintln!(
            "donna microsoft sync: teams chat cache stored (chats={})",
            chats.len()
        );
        Ok(())
    }

    fn filter_changed_chats(
        &self,
        store: &LocalStore,
        chats: Vec<GraphChat>,
    ) -> Result<Vec<GraphChat>, GraphError> {
        let mut changed = Vec::new();

        for chat in chats {
            let activity_at = chat.activity_at();
            let activity_state_key = format!("microsoft.teams.chat.activity_at.{}", chat.id);
            let last_synced_activity_at = store
                .runtime_state(&activity_state_key)?
                .and_then(|value| value.parse::<i64>().ok());
            if is_chat_unchanged_since_last_sync(activity_at, last_synced_activity_at) {
                continue;
            }
            changed.push(chat);
        }

        Ok(changed)
    }

    fn collect_joined_teams(&self) -> Result<Vec<GraphTeam>, GraphError> {
        let mut url = "https://graph.microsoft.com/v1.0/me/joinedTeams".to_owned();
        let mut teams = Vec::new();

        loop {
            let page: GraphPage<GraphTeam> = self.get(&url)?;
            teams.extend(page.value);
            match page.next_link {
                Some(next_link) => url = next_link,
                None => break,
            }
        }

        Ok(teams)
    }

    fn collect_joined_team_channels(
        &self,
        teams: &[GraphTeam],
    ) -> Result<Vec<GraphChannelRef>, GraphError> {
        let mut channels = Vec::new();

        for team in teams {
            let mut url = format!("https://graph.microsoft.com/v1.0/teams/{}/allChannels", team.id);
            loop {
                let page: GraphPage<GraphChannel> = self.get(&url)?;
                channels.extend(page.value.into_iter().map(|channel| GraphChannelRef {
                    team_id: team.id.clone(),
                    channel_id: channel.id,
                }));
                match page.next_link {
                    Some(next_link) => url = next_link,
                    None => break,
                }
            }
        }

        Ok(channels)
    }

    fn collect_cached_or_fallback_team_channels(
        &self,
        store: &LocalStore,
    ) -> Result<Vec<GraphChannelRef>, GraphError> {
        let now = now_seconds()?;
        if let Some(channels) = self.load_cached_team_channels(store, now)? {
            return Ok(channels);
        }

        match self.collect_live_team_channels() {
            Ok(channels) => {
                self.store_cached_team_channels(store, &channels, now)?;
                Ok(channels)
            }
            Err(GraphError::TeamsPermissionUnavailable { .. }) | Err(GraphError::Auth(_)) => {
                let channels = self.collect_cached_or_local_team_channels(store)?;
                eprintln!(
                    "donna microsoft sync: teams channel using local conversation fallback (channels={})",
                    channels.len()
                );
                Ok(channels)
            }
            Err(error) => Err(error),
        }
    }

    fn collect_live_team_channels(&self) -> Result<Vec<GraphChannelRef>, GraphError> {
        let teams = self.collect_joined_teams()?;
        self.collect_joined_team_channels(&teams)
    }

    fn collect_cached_or_local_team_channels(
        &self,
        store: &LocalStore,
    ) -> Result<Vec<GraphChannelRef>, GraphError> {
        let conversations = store.list_all_teams_channel_conversations()?;
        Ok(conversations
            .into_iter()
            .filter_map(|conversation| {
                let (team_id, channel_id) = conversation.split_once('/')?;
                if team_id.is_empty() || channel_id.is_empty() {
                    return None;
                }
                Some(GraphChannelRef {
                    team_id: team_id.to_owned(),
                    channel_id: channel_id.to_owned(),
                })
            })
            .collect())
    }

    fn load_cached_team_channels(
        &self,
        store: &LocalStore,
        now: i64,
    ) -> Result<Option<Vec<GraphChannelRef>>, GraphError> {
        let Some(last_at) = store.runtime_state(TEAMS_CHANNEL_CACHE_LAST_AT_KEY)? else {
            return Ok(None);
        };
        let Ok(last_at) = last_at.parse::<i64>() else {
            return Ok(None);
        };
        if now - last_at >= TEAMS_CHANNEL_CACHE_SECONDS {
            return Ok(None);
        }

        let Some(raw) = store.runtime_state(TEAMS_CHANNEL_CACHE_KEY)? else {
            return Ok(None);
        };
        let channels = raw
            .lines()
            .filter_map(|line| {
                let (team_id, channel_id) = line.split_once('\t')?;
                if team_id.is_empty() || channel_id.is_empty() {
                    return None;
                }
                Some(GraphChannelRef {
                    team_id: team_id.to_owned(),
                    channel_id: channel_id.to_owned(),
                })
            })
            .collect::<Vec<_>>();
        if channels.is_empty() {
            return Ok(None);
        }

        eprintln!(
            "donna microsoft sync: teams channel cache hit (channels={}, age_seconds={})",
            channels.len(),
            now - last_at
        );
        Ok(Some(channels))
    }

    fn store_cached_team_channels(
        &self,
        store: &LocalStore,
        channels: &[GraphChannelRef],
        now: i64,
    ) -> Result<(), GraphError> {
        let raw = channels
            .iter()
            .map(|channel| format!("{}\t{}", channel.team_id, channel.channel_id))
            .collect::<Vec<_>>()
            .join("\n");
        store.set_runtime_state(TEAMS_CHANNEL_CACHE_KEY, &raw)?;
        store.set_runtime_state(TEAMS_CHANNEL_CACHE_LAST_AT_KEY, &now.to_string())?;
        eprintln!(
            "donna microsoft sync: teams channel cache stored (channels={})",
            channels.len()
        );
        Ok(())
    }

    fn sync_channel_messages_for_channel(
        &self,
        store: &LocalStore,
        channel: &GraphChannelRef,
        activity_cutoff: i64,
    ) -> Result<usize, GraphError> {
        let activity_key = format!(
            "microsoft.teams.channel.activity_at.{}/{}",
            channel.team_id, channel.channel_id
        );
        let last_synced_activity_at = store
            .runtime_state(&activity_key)?
            .and_then(|value| value.parse::<i64>().ok());
        let message_cutoff = incremental_message_cutoff(activity_cutoff, last_synced_activity_at);
        let conversation_id = format!("{}/{}", channel.team_id, channel.channel_id);
        let mut url = format!(
            "https://graph.microsoft.com/v1.0/teams/{}/channels/{}/messages?$top=50",
            channel.team_id, channel.channel_id
        );
        let mut fetched = 0usize;
        let mut latest_activity = None;

        loop {
            let page: GraphPage<GraphTeamsMessage> = self.get(&url)?;
            let mut reached_cutoff = false;

            for message in page.value {
                let activity_at = message.activity_at();
                if latest_activity.is_none() {
                    latest_activity = activity_at;
                }
                if !message_is_new_enough(activity_at, message_cutoff) {
                    reached_cutoff = true;
                    break;
                }
                store.upsert_teams_message(&teams_message_from_graph(message, &conversation_id))?;
                fetched += 1;
            }

            if reached_cutoff {
                break;
            }

            match page.next_link {
                Some(next_link) => url = next_link,
                None => break,
            }
        }

        if let Some(activity_at) = latest_activity {
            store.set_runtime_state(&activity_key, &activity_at.to_string())?;
        }

        Ok(fetched)
    }

    fn process_pages<T, F>(
        &self,
        store: &LocalStore,
        source: &str,
        log_label: &str,
        initial_url: String,
        _progress_total: Option<usize>,
        mut persist: F,
    ) -> Result<usize, GraphError>
    where
        T: for<'de> Deserialize<'de>,
        F: FnMut(T) -> Result<(), GraphError>,
    {
        let mut url = initial_url;
        let mut pages = 0usize;
        let mut records = 0usize;
        let mut last_page_records;

        loop {
            let page: GraphPage<T> = self.get(&url)?;
            pages += 1;
            let page_records = page.value.len();
            last_page_records = page_records;
            for record in page.value {
                persist(record)?;
                records += 1;
            }
            write_sync_progress(store, source, page.next_link.clone(), page.delta_link.clone())?;
            if source == TEAMS_CHANNEL_SOURCE {
                eprintln!(
                    "donna microsoft sync: teams channel progress (pages={pages}, records={records})"
                );
            }

            match page.next_link {
                Some(next_link) => url = next_link,
                None => break,
            }
        }

        eprintln!(
            "donna microsoft sync: page persisted (source={source}, stream={log_label}, pages={pages}, records={records}, last_page={})",
            last_page_records,
        );

        Ok(records)
    }

    fn get<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T, GraphError> {
        let response = self
            .http
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", self.access_token))
            .header("Prefer", "odata.maxpagesize=100")
            .send()?;
        let status = response.status();
        let body = response.text()?;

        if status.is_success() {
            return serde_json::from_str(&body)
                .map_err(|error| GraphError::UnexpectedResponse(error.to_string()));
        }

        let error = serde_json::from_str::<GraphErrorBody>(&body).ok();
        Err(GraphError::auth_error(
            error.as_ref().map(|e| e.error.code.as_str()).unwrap_or("graph_error"),
            error
                .as_ref()
                .and_then(|e| e.error.message.as_deref())
                .or(Some(body.as_str())),
        ))
    }
}

fn teams_message_from_graph(item: GraphTeamsMessage, fallback_chat_id: &str) -> NewTeamsMessage {
    NewTeamsMessage {
        external_id: item.id,
        chat_id: item.chat_id.unwrap_or_else(|| fallback_chat_id.to_owned()),
        sender_name: item
            .from
            .as_ref()
            .and_then(|from| from.user.as_ref().and_then(|user| user.display_name.clone())),
        sender_external_id: item
            .from
            .as_ref()
            .and_then(|from| from.user.as_ref().and_then(|user| user.id.clone())),
        body: item
            .body
            .as_ref()
            .and_then(|body| body.content.clone())
            .unwrap_or_default(),
        importance: item.importance,
        web_url: item.web_url,
        sent_at: item.created_date_time.as_deref().and_then(parse_graph_time),
        etag: item.etag,
        change_key: None,
        is_deleted: false,
    }
}

fn write_sync_progress(
    store: &LocalStore,
    source: &str,
    cursor: Option<String>,
    delta_link: Option<String>,
) -> Result<(), GraphError> {
    let previous = store.sync_state(source)?;
    store.upsert_sync_state(&NewSyncState {
        source: source.to_owned(),
        cursor,
        delta_link: delta_link.or_else(|| previous.and_then(|state| state.delta_link)),
        last_sync_at: Some(now_seconds()?),
        last_error: None,
        is_stale: false,
    })?;
    Ok(())
}

pub fn mark_stale(store: &LocalStore, source: &str, reason: &str) -> Result<(), GraphError> {
    let previous = store.sync_state(source)?;
    store.upsert_sync_state(&NewSyncState {
        source: source.to_owned(),
        cursor: previous.as_ref().and_then(|state| state.cursor.clone()),
        delta_link: previous.as_ref().and_then(|state| state.delta_link.clone()),
        last_sync_at: previous.as_ref().and_then(|state| state.last_sync_at),
        last_error: Some(reason.to_owned()),
        is_stale: true,
    })?;
    Ok(())
}

/// A `reqwest::blocking::Client` owns a dedicated background thread for as
/// long as it's alive. `GraphSyncClient::new` used to build a fresh one on
/// every call, and since a full Microsoft sync runs once a minute in the
/// background, that churned a new thread every minute that wasn't guaranteed
/// to wind down before the next one was created. Build the underlying
/// client once and clone it (an `Arc` clone, not a new thread) into each
/// `GraphSyncClient`.
pub(crate) fn shared_http_client() -> &'static Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new())
    })
}

fn now_seconds() -> Result<i64, GraphError> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    Ok(elapsed.as_secs() as i64)
}

fn parse_graph_time(value: &str) -> Option<i64> {
    let trimmed = value.trim();
    let core = trimmed.strip_suffix('Z').unwrap_or(trimmed);
    let (date, time) = core.split_once('T')?;
    let mut d = date.split('-');
    let year: i32 = d.next()?.parse().ok()?;
    let month: u8 = d.next()?.parse().ok()?;
    let day: u8 = d.next()?.parse().ok()?;

    let mut t = time.split(':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let second: i64 = t.next()?.split('.').next()?.parse().ok()?;

    let days = days_from_civil(year, month, day);
    Some(days * DAY_SECONDS + (hour * 3_600) + (minute * 60) + second)
}

fn format_graph_time(seconds: i64) -> String {
    let days = seconds.div_euclid(DAY_SECONDS);
    let sod = seconds.rem_euclid(DAY_SECONDS);
    let (year, month, day) = civil_from_days(days);
    let hour = sod / 3_600;
    let minute = (sod % 3_600) / 60;
    let second = sod % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u8, day as u8)
}

fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let m = month as i64;
    let d = day as i64;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn graph_time_gt_filter(field: &str, timestamp: i64) -> String {
    if timestamp <= 0 {
        return format!("{field}%20ge%201970-01-01T00:00:00Z");
    }

    format!("{field}%20gt%20{}", format_graph_time(timestamp))
}

fn percent_progress(done: usize, total: usize) -> usize {
    if total == 0 {
        return 100;
    }

    ((done.saturating_mul(100)) / total).min(100)
}

fn is_chat_unchanged_since_last_sync(
    activity_at: Option<i64>,
    last_synced_activity_at: Option<i64>,
) -> bool {
    match (activity_at, last_synced_activity_at) {
        (Some(current), Some(previous)) => current <= previous,
        _ => false,
    }
}

fn incremental_message_cutoff(activity_cutoff: i64, last_synced_activity_at: Option<i64>) -> i64 {
    match last_synced_activity_at {
        Some(previous) if previous > activity_cutoff => previous,
        _ => activity_cutoff,
    }
}

fn message_is_new_enough(activity_at: Option<i64>, cutoff: i64) -> bool {
    if cutoff <= 0 {
        return true;
    }

    activity_at.is_some_and(|timestamp| timestamp > cutoff)
}

fn should_prune(store: &LocalStore, key: &str, now: i64) -> Result<bool, GraphError> {
    let previous = store.runtime_state(key)?;
    let should = match previous {
        Some(value) => value
            .parse::<i64>()
            .map(|timestamp| now - timestamp >= 3_600)
            .unwrap_or(true),
        None => true,
    };
    Ok(should)
}

#[derive(Debug, Deserialize)]
struct GraphPage<T> {
    value: Vec<T>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
    #[serde(rename = "@odata.deltaLink")]
    delta_link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphErrorBody {
    error: GraphErrorInner,
}

#[derive(Debug, Deserialize)]
struct GraphErrorInner {
    code: String,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphChat {
    id: String,
    #[serde(rename = "lastMessagePreview")]
    last_message_preview: Option<GraphChatMessagePreview>,
    #[serde(rename = "lastUpdatedDateTime")]
    last_updated_date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphTeam {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GraphChannel {
    id: String,
}

#[derive(Debug, Clone)]
struct GraphChannelRef {
    team_id: String,
    channel_id: String,
}

impl GraphChat {
    fn has_recent_activity(&self, cutoff: i64) -> bool {
        if cutoff <= 0 {
            return true;
        }

        self.last_updated_date_time
            .as_deref()
            .and_then(parse_graph_time)
            .or_else(|| {
                self.last_message_preview
                    .as_ref()
                    .and_then(|preview| preview.created_date_time.as_deref())
                    .and_then(parse_graph_time)
            })
            .map(|timestamp| timestamp >= cutoff)
            .unwrap_or(true)
    }

    fn activity_at(&self) -> Option<i64> {
        self.last_updated_date_time
            .as_deref()
            .and_then(parse_graph_time)
            .or_else(|| {
                self.last_message_preview
                    .as_ref()
                    .and_then(|preview| preview.created_date_time.as_deref())
                    .and_then(parse_graph_time)
            })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphChatMessagePreview {
    created_date_time: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphMail {
    id: String,
    parent_folder_id: Option<String>,
    subject: Option<String>,
    sender: Option<GraphSender>,
    body_preview: Option<String>,
    received_date_time: Option<String>,
    #[serde(rename = "@odata.etag")]
    etag: Option<String>,
    change_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphSender {
    email_address: Option<GraphEmailAddress>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphEmailAddress {
    name: Option<String>,
    address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphCalendarEventDetail {
    subject: Option<String>,
    organizer: Option<GraphSender>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphTeamsMessage {
    id: String,
    chat_id: Option<String>,
    from: Option<GraphIdentitySet>,
    body: Option<GraphItemBody>,
    importance: Option<String>,
    web_url: Option<String>,
    created_date_time: Option<String>,
    last_modified_date_time: Option<String>,
    #[serde(rename = "@odata.etag")]
    etag: Option<String>,
}

impl GraphTeamsMessage {
    fn activity_at(&self) -> Option<i64> {
        self.last_modified_date_time
            .as_deref()
            .and_then(parse_graph_time)
            .or_else(|| self.created_date_time.as_deref().and_then(parse_graph_time))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphIdentitySet {
    user: Option<GraphIdentity>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphIdentity {
    id: Option<String>,
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphItemBody {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphCalendarEvent {
    id: String,
    subject: Option<String>,
    organizer: Option<GraphSender>,
    start: Option<GraphDateTimeTimeZone>,
    end: Option<GraphDateTimeTimeZone>,
    original_start_time_zone: Option<String>,
    show_as: Option<String>,
    is_cancelled: Option<bool>,
    is_all_day: Option<bool>,
    #[serde(default)]
    attendees: Vec<GraphAttendee>,
    #[serde(rename = "@odata.etag")]
    etag: Option<String>,
    change_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphAttendee {
    email_address: Option<GraphEmailAddress>,
    #[serde(rename = "type")]
    attendee_type: Option<String>,
}

impl GraphAttendee {
    fn into_calendar_attendee(self) -> Option<CalendarAttendee> {
        let email_address = self.email_address?;
        if email_address.name.is_none() && email_address.address.is_none() {
            return None;
        }
        Some(CalendarAttendee {
            name: email_address.name,
            email: email_address.address,
            is_optional: self.attendee_type.as_deref() == Some("optional"),
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphDateTimeTimeZone {
    date_time: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{
        GraphChat, GraphChatMessagePreview, format_graph_time, incremental_message_cutoff,
        is_chat_unchanged_since_last_sync, parse_graph_time,
    };

    #[test]
    fn graph_time_round_trip_is_stable() {
        let timestamp = 1_700_000_123;
        let encoded = format_graph_time(timestamp);
        let decoded = parse_graph_time(&encoded).expect("decode");
        assert_eq!(decoded, timestamp);
    }

    #[test]
    fn chat_is_unchanged_when_activity_did_not_advance() {
        assert!(is_chat_unchanged_since_last_sync(Some(200), Some(200)));
        assert!(is_chat_unchanged_since_last_sync(Some(199), Some(200)));
        assert!(!is_chat_unchanged_since_last_sync(Some(201), Some(200)));
        assert!(!is_chat_unchanged_since_last_sync(Some(200), None));
    }

    #[test]
    fn incremental_cutoff_uses_newer_chat_watermark() {
        assert_eq!(incremental_message_cutoff(100, Some(150)), 150);
        assert_eq!(incremental_message_cutoff(100, Some(100)), 100);
        assert_eq!(incremental_message_cutoff(100, Some(50)), 100);
        assert_eq!(incremental_message_cutoff(100, None), 100);
    }

    #[test]
    fn chat_activity_prefers_last_updated_and_falls_back_to_preview() {
        let updated = "2024-01-03T12:00:00Z";
        let preview = "2024-01-02T12:00:00Z";
        let chat = GraphChat {
            id: "chat-1".to_owned(),
            last_message_preview: Some(GraphChatMessagePreview {
                created_date_time: Some(preview.to_owned()),
            }),
            last_updated_date_time: Some(updated.to_owned()),
        };
        let fallback = GraphChat {
            id: "chat-2".to_owned(),
            last_message_preview: Some(GraphChatMessagePreview {
                created_date_time: Some(preview.to_owned()),
            }),
            last_updated_date_time: None,
        };

        assert_eq!(chat.activity_at(), parse_graph_time(updated));
        assert_eq!(fallback.activity_at(), parse_graph_time(preview));
    }

}
