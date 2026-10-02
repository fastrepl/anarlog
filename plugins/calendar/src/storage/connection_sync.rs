use std::collections::HashSet;

use anlg_calendar_interface::CalendarProviderType;
use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use indexmap::IndexMap;

use super::identity::js_date_parse_millis;
use super::participants::{
    ParticipantSyncSnapshot, ParticipantsSyncOutput, sync_session_participants,
};
use super::provider_str;
use super::transaction_utils::{js_iso8601_timestamp, rollback_error};

const LEGACY_MAIN_VALUES_ID: &str = "legacy_main_values_document";
const LEGACY_SETTINGS_ID: &str = "legacy_settings_document";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SyncCalendarRef {
    pub id: String,
    pub tracking_id_calendar: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct IncomingCalendarEvent {
    pub tracking_id_event: String,
    pub tracking_id_calendar: String,
    #[serde(default)]
    pub legacy_tracking_ids: Vec<String>,
    #[serde(default)]
    pub is_cancelled: bool,
    pub title: Option<String>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub location: Option<String>,
    pub meeting_link: Option<String>,
    pub description: Option<String>,
    pub recurrence_series_id: Option<String>,
    pub has_recurrence_rules: bool,
    pub is_all_day: bool,
    #[serde(default)]
    pub attendance_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct IncomingEventParticipant {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(default)]
    pub is_organizer: bool,
    #[serde(default)]
    pub is_current_user: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct IncomingEventParticipants {
    pub tracking_id_event: String,
    pub participants: Vec<IncomingEventParticipant>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct SyncCalendarConnectionEventsRequest {
    pub provider: CalendarProviderType,
    pub connection_id: String,
    pub from: String,
    pub to: String,
    pub calendars: Vec<SyncCalendarRef>,
    pub events: Vec<IncomingCalendarEvent>,
    pub participants: Vec<IncomingEventParticipants>,
}

pub async fn sync_calendar_connection_events(
    pool: &SqlitePool,
    request: SyncCalendarConnectionEventsRequest,
) -> Result<(), String> {
    let provider = provider_str(request.provider);
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let result = run(&mut transaction, &request, provider, &now).await;
    if let Err(error) = result {
        return Err(rollback_error(transaction, error).await);
    }

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())
}

// Port of the TS `runForConnection` post-fetch pipeline: the diff, session
// link updates, participant reconciliation, and all writes in one transaction.
async fn run(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    request: &SyncCalendarConnectionEventsRequest,
    provider: &str,
    now: &str,
) -> Result<(), String> {
    // createCtx equivalent, built from the calendars list in load order.
    let mut calendar_ids: Vec<String> = Vec::new();
    let mut tracking_to_id: IndexMap<String, String> = IndexMap::new();
    for calendar in &request.calendars {
        calendar_ids.push(calendar.id.clone());
        if !calendar.tracking_id_calendar.is_empty() {
            tracking_to_id.insert(calendar.tracking_id_calendar.clone(), calendar.id.clone());
        }
    }
    let ctx = SyncCtx {
        calendar_ids,
        tracking_to_id,
    };

    let mut tracking_ids: Vec<String> = Vec::new();
    let mut seen_tracking_ids: HashSet<&str> = HashSet::new();
    for event in &request.events {
        for tracking_id in event_tracking_ids(event) {
            if seen_tracking_ids.insert(tracking_id) {
                tracking_ids.push(tracking_id.to_string());
            }
        }
    }

    let existing = anlg_db_app::list_events_for_sync(
        transaction,
        &ctx.calendar_ids,
        &request.to,
        &request.from,
        &tracking_ids,
    )
    .await
    .map_err(|error| error.to_string())?;

    let mut incoming_participants: IndexMap<String, Vec<IncomingEventParticipant>> =
        IndexMap::new();
    for entry in &request.participants {
        incoming_participants.insert(entry.tracking_id_event.clone(), entry.participants.clone());
    }

    let events = sync_events(
        &ctx,
        &request.events,
        &existing,
        &incoming_participants,
        &request.from,
        &request.to,
    );

    let sessions = anlg_db_app::list_sessions_for_tracking_ids(transaction, &tracking_ids)
        .await
        .map_err(|error| error.to_string())?;

    let session_updates = sync_session_embedded_events(&ctx, &request.events, &sessions);
    let session_tracking_ids: std::collections::HashMap<String, String> = session_updates
        .iter()
        .map(|update| (update.session_id.clone(), update.tracking_id.clone()))
        .collect();
    let snapshot_sessions: Vec<anlg_db_app::SessionSyncRow> = sessions
        .into_iter()
        .filter(|session| session_tracking_ids.contains_key(&session.id))
        .map(|mut session| {
            session.tracking_id = session_tracking_ids[&session.id].clone();
            session
        })
        .collect();

    let session_ids: Vec<String> = snapshot_sessions
        .iter()
        .map(|session| session.id.clone())
        .collect();
    let mut emails: Vec<String> = Vec::new();
    let mut seen_emails: HashSet<String> = HashSet::new();
    for participants in incoming_participants.values() {
        for participant in participants {
            if let Some(email) = participant
                .email
                .as_deref()
                .map(str::trim)
                .filter(|email| !email.is_empty())
                .map(|email| email.to_lowercase())
                && seen_emails.insert(email.clone())
            {
                emails.push(email);
            }
        }
    }

    let humans = anlg_db_app::list_humans_by_emails(transaction, &emails)
        .await
        .map_err(|error| error.to_string())?;
    let mappings = anlg_db_app::list_session_participant_mappings(transaction, &session_ids)
        .await
        .map_err(|error| error.to_string())?;

    let participants = sync_session_participants(
        &incoming_participants,
        ParticipantSyncSnapshot {
            sessions: snapshot_sessions,
            humans: &humans,
            mappings: &mappings,
        },
    );

    // applyConnectionSync: the statements below keep the TS order.
    if provider == "apple"
        && let Some(aliases_json) = ignored_event_aliases_json(
            events
                .to_update
                .iter()
                .map(|event| {
                    (
                        event.tracking_id_event.clone(),
                        event.legacy_tracking_ids.clone(),
                    )
                })
                .chain(events.to_add.iter().map(|event| {
                    (
                        event.tracking_id_event.clone(),
                        event.legacy_tracking_ids.clone(),
                    )
                })),
        )
    {
        anlg_db_app::migrate_ignored_event_ids(
            transaction,
            LEGACY_MAIN_VALUES_ID,
            LEGACY_SETTINGS_ID,
            &aliases_json,
            now,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    for event_id in &events.to_delete {
        anlg_db_app::soft_delete_event(transaction, now, event_id)
            .await
            .map_err(|error| error.to_string())?;
    }

    let mut event_ids_by_key: IndexMap<String, String> = IndexMap::new();
    for event in &events.to_update {
        event_ids_by_key.insert(
            storage_event_key(&event.calendar_id, &event.tracking_id_event),
            event.id.clone(),
        );
        anlg_db_app::update_synced_event(
            transaction,
            &event.id,
            &event.tracking_id_event,
            &event.calendar_id,
            event.title.as_deref().unwrap_or(""),
            event.started_at.as_deref().unwrap_or(""),
            event.ended_at.as_deref().unwrap_or(""),
            event.location.as_deref().unwrap_or(""),
            event.meeting_link.as_deref().unwrap_or(""),
            event.description.as_deref().unwrap_or(""),
            event.recurrence_series_id.as_deref().unwrap_or(""),
            event.has_recurrence_rules,
            event.is_all_day,
            provider,
            encode_participants(&event.participants).as_deref(),
            event.attendance_json.as_deref(),
            now,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    for event in &events.to_add {
        let Some(calendar_id) = ctx.tracking_to_id.get(&event.tracking_id_calendar) else {
            continue;
        };
        let event_id = uuid::Uuid::new_v4().to_string();
        event_ids_by_key.insert(
            storage_event_key(calendar_id, &event.tracking_id_event),
            event_id.clone(),
        );
        anlg_db_app::insert_synced_event(
            transaction,
            &event_id,
            &event.tracking_id_event,
            calendar_id,
            event.title.as_deref().unwrap_or(""),
            event.started_at.as_deref().unwrap_or(""),
            event.ended_at.as_deref().unwrap_or(""),
            event.location.as_deref().unwrap_or(""),
            event.meeting_link.as_deref().unwrap_or(""),
            event.description.as_deref().unwrap_or(""),
            event.recurrence_series_id.as_deref().unwrap_or(""),
            event.has_recurrence_rules,
            event.is_all_day,
            provider,
            encode_participants(&event.participants).as_deref(),
            event.attendance_json.as_deref(),
            now,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    for update in &session_updates {
        let event_id = event_ids_by_key
            .get(&storage_event_key(&update.calendar_id, &update.tracking_id))
            .cloned()
            .unwrap_or_default();
        anlg_db_app::update_session_for_synced_event(
            transaction,
            &update.session_id,
            &event_id,
            &update.tracking_id,
            provider,
            &update.series_id,
            &update.event_json,
            now,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    apply_participant_writes(transaction, &participants, now).await?;

    Ok(())
}

struct CompanyPlan {
    name: String,
    owner_user_id: String,
    new_human_emails: Vec<String>,
    enrich_human_ids: Vec<String>,
}

fn plan_company<'a>(
    company_names: &'a mut IndexMap<String, CompanyPlan>,
    company_name: Option<&String>,
    owner_user_id: &str,
) -> Option<&'a mut CompanyPlan> {
    let company_name = company_name?;
    let key = company_name.to_lowercase();
    if !company_names.contains_key(&key) {
        company_names.insert(
            key.clone(),
            CompanyPlan {
                name: company_name.clone(),
                owner_user_id: owner_user_id.to_string(),
                new_human_emails: Vec::new(),
                enrich_human_ids: Vec::new(),
            },
        );
    }
    company_names.get_mut(&key)
}

async fn apply_participant_writes(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    participants: &ParticipantsSyncOutput,
    now: &str,
) -> Result<(), String> {
    let mut company_names: IndexMap<String, CompanyPlan> = IndexMap::new();
    for human in &participants.humans_to_create {
        if let Some(plan) = plan_company(
            &mut company_names,
            human.company_name.as_ref(),
            &human.owner_user_id,
        ) {
            plan.new_human_emails.push(human.email.to_lowercase());
        }
    }
    for human in &participants.humans_to_enrich {
        if let Some(plan) = plan_company(
            &mut company_names,
            human.company_name.as_ref(),
            &human.owner_user_id,
        ) {
            plan.enrich_human_ids.push(human.id.clone());
        }
    }
    for (_, company) in company_names.iter() {
        anlg_db_app::insert_organization_if_still_needed(
            transaction,
            &uuid::Uuid::new_v4().to_string(),
            &company.owner_user_id,
            &company.name,
            now,
            &company.new_human_emails,
            &company.enrich_human_ids,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    for human in &participants.humans_to_create {
        anlg_db_app::insert_human_if_missing(
            transaction,
            &human.id,
            &human.owner_user_id,
            &human.name,
            &human.email,
            human.company_name.as_deref().unwrap_or(""),
            now,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    for human in &participants.humans_to_enrich {
        anlg_db_app::update_human_enrichment(
            transaction,
            &human.id,
            human.name.as_deref(),
            human.company_name.as_deref(),
            now,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    for mapping_id in &participants.to_delete {
        anlg_db_app::soft_delete_auto_participant_mapping(transaction, now, mapping_id)
            .await
            .map_err(|error| error.to_string())?;
    }

    for mapping in &participants.to_add {
        anlg_db_app::insert_session_participant_mapping(
            transaction,
            &uuid::Uuid::new_v4().to_string(),
            now,
            &mapping.human_id,
            &mapping.email,
            &mapping.session_id,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub(crate) struct SyncCtx {
    pub calendar_ids: Vec<String>,
    pub tracking_to_id: IndexMap<String, String>,
}

fn event_key(calendar_id: &str, tracking_id: &str) -> String {
    serde_json::to_string(&(calendar_id, tracking_id)).unwrap_or_default()
}

fn storage_event_key(calendar_id: &str, tracking_id: &str) -> String {
    format!("{calendar_id}\u{0}{tracking_id}")
}

fn ignored_event_aliases_json(
    events: impl Iterator<Item = (String, Vec<String>)>,
) -> Option<String> {
    let mut aliases: IndexMap<String, String> = IndexMap::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    for (tracking_id_event, legacy_ids) in events {
        for legacy_id in legacy_ids {
            if legacy_id == tracking_id_event {
                continue;
            }
            if let Some(current) = aliases.get(&legacy_id)
                && *current != tracking_id_event
                && !ambiguous.contains(&legacy_id)
            {
                ambiguous.insert(legacy_id.clone());
            }
            aliases.insert(legacy_id, tracking_id_event.clone());
        }
    }
    for legacy_id in ambiguous {
        aliases.shift_remove(&legacy_id);
    }
    if aliases.is_empty() {
        return None;
    }
    let pairs: Vec<(String, String)> = aliases
        .iter()
        .map(|(old, new)| (old.clone(), new.clone()))
        .collect();
    serde_json::to_string(&pairs).ok()
}

fn event_tracking_ids(event: &IncomingCalendarEvent) -> Vec<&str> {
    let mut ids = vec![event.tracking_id_event.as_str()];
    ids.extend(event.legacy_tracking_ids.iter().map(String::as_str));
    ids
}

fn index_incoming_events<'a>(
    ctx: &SyncCtx,
    incoming: &'a [IncomingCalendarEvent],
) -> IndexMap<String, &'a IncomingCalendarEvent> {
    let mut index: IndexMap<String, &IncomingCalendarEvent> = IndexMap::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    for event in incoming {
        let Some(calendar_id) = ctx.tracking_to_id.get(&event.tracking_id_calendar) else {
            continue;
        };
        for tracking_id in event_tracking_ids(event) {
            let key = event_key(calendar_id, tracking_id);
            if let Some(current) = index.get(&key)
                && current.tracking_id_event != event.tracking_id_event
                && !ambiguous.contains(&key)
            {
                ambiguous.insert(key.clone());
            }
            index.insert(key, event);
        }
    }
    for key in ambiguous {
        index.shift_remove(&key);
    }
    index
}

#[derive(Debug)]
struct EventToUpdate {
    id: String,
    tracking_id_event: String,
    calendar_id: String,
    title: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
    location: Option<String>,
    meeting_link: Option<String>,
    description: Option<String>,
    recurrence_series_id: Option<String>,
    has_recurrence_rules: bool,
    is_all_day: bool,
    attendance_json: Option<String>,
    legacy_tracking_ids: Vec<String>,
    participants: Vec<IncomingEventParticipant>,
}

#[derive(Debug)]
struct EventToAdd {
    tracking_id_event: String,
    tracking_id_calendar: String,
    title: Option<String>,
    started_at: Option<String>,
    ended_at: Option<String>,
    location: Option<String>,
    meeting_link: Option<String>,
    description: Option<String>,
    recurrence_series_id: Option<String>,
    has_recurrence_rules: bool,
    is_all_day: bool,
    attendance_json: Option<String>,
    legacy_tracking_ids: Vec<String>,
    participants: Vec<IncomingEventParticipant>,
}

#[derive(Debug, Default)]
struct EventsSync {
    to_delete: Vec<String>,
    to_update: Vec<EventToUpdate>,
    to_add: Vec<EventToAdd>,
}

fn sync_events(
    ctx: &SyncCtx,
    incoming: &[IncomingCalendarEvent],
    existing: &[anlg_db_app::EventSyncRow],
    incoming_participants: &IndexMap<String, Vec<IncomingEventParticipant>>,
    from: &str,
    to: &str,
) -> EventsSync {
    let mut out = EventsSync::default();
    let from_ms = js_date_parse_millis(from);
    let to_ms = js_date_parse_millis(to);

    let incoming_by_key = index_incoming_events(ctx, incoming);
    let mut handled_keys: HashSet<String> = HashSet::new();

    for store_event in existing {
        let tracking_id = store_event.tracking_id_event.as_str();
        let matching = incoming_by_key.get(&event_key(&store_event.calendar_id, tracking_id));
        let key = event_key(
            &store_event.calendar_id,
            matching
                .map(|event| event.tracking_id_event.as_str())
                .unwrap_or(tracking_id),
        );

        if let Some(matching) = matching
            && !matching.is_cancelled
            && !handled_keys.contains(&key)
        {
            out.to_update.push(EventToUpdate {
                id: store_event.id.clone(),
                tracking_id_event: matching.tracking_id_event.clone(),
                calendar_id: store_event.calendar_id.clone(),
                title: matching
                    .title
                    .clone()
                    .or_else(|| Some(store_event.title.clone())),
                started_at: matching
                    .started_at
                    .clone()
                    .or_else(|| Some(store_event.started_at.clone())),
                ended_at: matching
                    .ended_at
                    .clone()
                    .or_else(|| Some(store_event.ended_at.clone())),
                location: matching.location.clone(),
                meeting_link: matching.meeting_link.clone(),
                description: matching.description.clone(),
                recurrence_series_id: matching.recurrence_series_id.clone(),
                has_recurrence_rules: matching.has_recurrence_rules,
                is_all_day: matching.is_all_day,
                attendance_json: matching.attendance_json.clone(),
                legacy_tracking_ids: matching.legacy_tracking_ids.clone(),
                participants: incoming_participants
                    .get(&matching.tracking_id_event)
                    .cloned()
                    .unwrap_or_default(),
            });
            handled_keys.insert(key);
            continue;
        }

        let overlaps_range = match (
            js_date_parse_millis(&store_event.started_at),
            js_date_parse_millis(if store_event.ended_at.is_empty() {
                &store_event.started_at
            } else {
                &store_event.ended_at
            }),
            from_ms,
            to_ms,
        ) {
            (Some(start), Some(end), Some(from_ms), Some(to_ms)) => {
                start <= to_ms && end >= from_ms
            }
            _ => false,
        };
        if store_event.deleted_at.is_none() && (matching.is_some() || overlaps_range) {
            out.to_delete.push(store_event.id.clone());
        }
    }

    let mut scheduled_keys: HashSet<String> = handled_keys.clone();
    for incoming_event in incoming {
        if incoming_event.is_cancelled {
            continue;
        }
        let calendar_id = ctx.tracking_to_id.get(&incoming_event.tracking_id_calendar);
        let key = calendar_id
            .map(|calendar_id| event_key(calendar_id, &incoming_event.tracking_id_event));
        if key.is_none() || !scheduled_keys.contains(key.as_ref().unwrap()) {
            out.to_add.push(EventToAdd {
                tracking_id_event: incoming_event.tracking_id_event.clone(),
                tracking_id_calendar: incoming_event.tracking_id_calendar.clone(),
                title: incoming_event.title.clone(),
                started_at: incoming_event.started_at.clone(),
                ended_at: incoming_event.ended_at.clone(),
                location: incoming_event.location.clone(),
                meeting_link: incoming_event.meeting_link.clone(),
                description: incoming_event.description.clone(),
                recurrence_series_id: incoming_event.recurrence_series_id.clone(),
                has_recurrence_rules: incoming_event.has_recurrence_rules,
                is_all_day: incoming_event.is_all_day,
                attendance_json: incoming_event.attendance_json.clone(),
                legacy_tracking_ids: incoming_event.legacy_tracking_ids.clone(),
                participants: incoming_participants
                    .get(&incoming_event.tracking_id_event)
                    .cloned()
                    .unwrap_or_default(),
            });
            if let Some(key) = key {
                scheduled_keys.insert(key);
            }
        }
    }

    out
}

#[derive(Debug)]
struct SessionEventUpdate {
    session_id: String,
    tracking_id: String,
    calendar_id: String,
    series_id: String,
    event_json: String,
}

#[derive(Serialize)]
struct SessionEventJson {
    tracking_id: String,
    calendar_id: String,
    title: String,
    started_at: String,
    ended_at: String,
    is_all_day: bool,
    has_recurrence_rules: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meeting_link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recurrence_series_id: Option<String>,
}

fn sync_session_embedded_events(
    ctx: &SyncCtx,
    incoming: &[IncomingCalendarEvent],
    sessions: &[anlg_db_app::SessionSyncRow],
) -> Vec<SessionEventUpdate> {
    let index = index_incoming_events(ctx, incoming);
    let mut updates = Vec::new();

    for session in sessions {
        let matches: Vec<&IncomingCalendarEvent> = if !session.calendar_id.is_empty() {
            index
                .get(&event_key(&session.calendar_id, &session.tracking_id))
                .copied()
                .into_iter()
                .collect()
        } else {
            ctx.calendar_ids
                .iter()
                .filter_map(|calendar_id| {
                    index
                        .get(&event_key(calendar_id, &session.tracking_id))
                        .copied()
                })
                .collect()
        };
        if matches.len() != 1 {
            continue;
        }
        let incoming_event = matches[0];
        if incoming_event.is_cancelled {
            continue;
        }

        let calendar_id = ctx
            .tracking_to_id
            .get(&incoming_event.tracking_id_calendar)
            .cloned()
            .unwrap_or_default();
        let event = SessionEventJson {
            tracking_id: incoming_event.tracking_id_event.clone(),
            calendar_id: calendar_id.clone(),
            title: incoming_event.title.clone().unwrap_or_default(),
            started_at: incoming_event.started_at.clone().unwrap_or_default(),
            ended_at: incoming_event.ended_at.clone().unwrap_or_default(),
            is_all_day: incoming_event.is_all_day,
            has_recurrence_rules: incoming_event.has_recurrence_rules,
            location: incoming_event.location.clone(),
            meeting_link: incoming_event.meeting_link.clone(),
            description: incoming_event.description.clone(),
            recurrence_series_id: incoming_event.recurrence_series_id.clone(),
        };

        updates.push(SessionEventUpdate {
            session_id: session.id.clone(),
            tracking_id: incoming_event.tracking_id_event.clone(),
            calendar_id,
            series_id: incoming_event
                .recurrence_series_id
                .clone()
                .unwrap_or_default(),
            event_json: serde_json::to_string(&event).unwrap_or_default(),
        });
    }

    updates
}

fn encode_participants(participants: &[IncomingEventParticipant]) -> Option<String> {
    if participants.is_empty() {
        None
    } else {
        serde_json::to_string(participants).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anlg_db_core::Db;
    use serde_json::json;

    fn ctx() -> SyncCtx {
        let mut tracking_to_id = IndexMap::new();
        tracking_to_id.insert("tracking-cal-1".to_string(), "cal-1".to_string());
        SyncCtx {
            calendar_ids: vec!["cal-1".to_string()],
            tracking_to_id,
        }
    }

    fn incoming_event(overrides: impl Fn(&mut IncomingCalendarEvent)) -> IncomingCalendarEvent {
        let mut event = IncomingCalendarEvent {
            tracking_id_event: "incoming-1".to_string(),
            tracking_id_calendar: "tracking-cal-1".to_string(),
            legacy_tracking_ids: vec![],
            is_cancelled: false,
            title: Some("Test Event".to_string()),
            started_at: Some("2024-01-15T10:00:00Z".to_string()),
            ended_at: Some("2024-01-15T11:00:00Z".to_string()),
            location: None,
            meeting_link: None,
            description: None,
            recurrence_series_id: None,
            has_recurrence_rules: false,
            is_all_day: false,
            attendance_json: None,
        };
        overrides(&mut event);
        event
    }

    fn existing_event(
        overrides: impl Fn(&mut anlg_db_app::EventSyncRow),
    ) -> anlg_db_app::EventSyncRow {
        let mut event = anlg_db_app::EventSyncRow {
            id: "event-1".to_string(),
            tracking_id_event: "existing-1".to_string(),
            calendar_id: "cal-1".to_string(),
            title: "Existing Event".to_string(),
            started_at: "2024-01-15T10:00:00Z".to_string(),
            ended_at: "2024-01-15T11:00:00Z".to_string(),
            location: String::new(),
            meeting_link: String::new(),
            description: String::new(),
            note: String::new(),
            recurrence_series_id: String::new(),
            has_recurrence_rules: false,
            is_all_day: false,
            provider: "apple".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            deleted_at: None,
        };
        overrides(&mut event);
        event
    }

    fn no_participants() -> IndexMap<String, Vec<IncomingEventParticipant>> {
        IndexMap::new()
    }

    const FROM: &str = "2024-01-01T00:00:00Z";
    const TO: &str = "2024-02-01T00:00:00Z";

    #[test]
    fn reconciles_a_moved_occurrence_and_its_duplicate_through_native_aliases() {
        let incoming = vec![incoming_event(|event| {
            event.tracking_id_event = "apple:occurrence-1".to_string();
            event.legacy_tracking_ids = vec![
                "store:uid:2024-01-15".to_string(),
                "store:uid/RID=727092000".to_string(),
            ];
            event.title = Some("Changed title".to_string());
            event.started_at = Some("2024-01-17T10:00:00Z".to_string());
            event.ended_at = Some("2024-01-17T11:00:00Z".to_string());
        })];
        let first = sync_events(
            &ctx(),
            &incoming,
            &[
                existing_event(|event| {
                    event.id = "linked-row".to_string();
                    event.tracking_id_event = "store:uid:2024-01-15".to_string();
                }),
                existing_event(|event| {
                    event.id = "duplicate-row".to_string();
                    event.tracking_id_event = "store:uid/RID=727092000".to_string();
                }),
            ],
            &no_participants(),
            FROM,
            TO,
        );
        assert!(first.to_add.is_empty());
        assert_eq!(first.to_delete, ["duplicate-row"]);
        assert_eq!(first.to_update.len(), 1);
        assert_eq!(first.to_update[0].id, "linked-row");
        assert_eq!(first.to_update[0].tracking_id_event, "apple:occurrence-1");
        assert_eq!(first.to_update[0].title.as_deref(), Some("Changed title"));

        let updated_rows: Vec<anlg_db_app::EventSyncRow> = first
            .to_update
            .iter()
            .map(|update| anlg_db_app::EventSyncRow {
                id: update.id.clone(),
                tracking_id_event: update.tracking_id_event.clone(),
                calendar_id: update.calendar_id.clone(),
                title: update.title.clone().unwrap_or_default(),
                started_at: update.started_at.clone().unwrap_or_default(),
                ended_at: update.ended_at.clone().unwrap_or_default(),
                location: update.location.clone().unwrap_or_default(),
                meeting_link: String::new(),
                description: String::new(),
                note: String::new(),
                recurrence_series_id: update.recurrence_series_id.clone().unwrap_or_default(),
                has_recurrence_rules: update.has_recurrence_rules,
                is_all_day: update.is_all_day,
                provider: "apple".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                deleted_at: None,
            })
            .collect();
        let second = sync_events(
            &ctx(),
            &incoming,
            &updated_rows,
            &no_participants(),
            FROM,
            TO,
        );
        assert!(second.to_add.is_empty());
        assert!(second.to_delete.is_empty());
        assert_eq!(second.to_update[0].id, "linked-row");
    }

    #[test]
    fn keeps_independent_events_with_identical_display_fields() {
        let result = sync_events(
            &ctx(),
            &[
                incoming_event(|event| event.tracking_id_event = "uid-A".to_string()),
                incoming_event(|event| event.tracking_id_event = "uid-B".to_string()),
            ],
            &[],
            &no_participants(),
            FROM,
            TO,
        );
        assert_eq!(result.to_add.len(), 2);
    }

    #[test]
    fn cancellation_deletes_an_aliased_row_even_outside_the_sync_window() {
        let result = sync_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "canonical".to_string();
                event.legacy_tracking_ids = vec!["existing-1".to_string()];
                event.is_cancelled = true;
            })],
            &[existing_event(|event| {
                event.started_at = "2023-12-15T10:00:00Z".to_string();
                event.ended_at = "2023-12-15T11:00:00Z".to_string();
            })],
            &no_participants(),
            FROM,
            TO,
        );
        assert!(result.to_add.is_empty());
        assert!(result.to_update.is_empty());
        assert_eq!(result.to_delete, ["event-1"]);
    }

    #[test]
    fn an_alias_from_another_calendar_cannot_update_or_delete_an_out_of_window_row() {
        let result = sync_events(
            &ctx(),
            &[incoming_event(|event| {
                event.legacy_tracking_ids = vec!["existing-1".to_string()];
            })],
            &[existing_event(|event| {
                event.calendar_id = "other".to_string();
                event.started_at = "2023-12-15T10:00:00Z".to_string();
                event.ended_at = "2023-12-15T11:00:00Z".to_string();
            })],
            &no_participants(),
            FROM,
            TO,
        );
        assert!(result.to_update.is_empty());
        assert!(result.to_delete.is_empty());
        assert_eq!(result.to_add.len(), 1);
    }

    #[test]
    fn updates_existing_events_with_matching_tracking_id() {
        let result = sync_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "existing-1".to_string()
            })],
            &[existing_event(|_| {})],
            &no_participants(),
            FROM,
            TO,
        );
        assert_eq!(result.to_update.len(), 1);
        assert!(result.to_add.is_empty());
        assert!(result.to_delete.is_empty());
    }

    #[test]
    fn deletes_orphaned_events_without_matching_incoming() {
        let result = sync_events(
            &ctx(),
            &[],
            &[existing_event(|_| {})],
            &no_participants(),
            FROM,
            TO,
        );
        assert_eq!(result.to_delete, ["event-1"]);
    }

    #[test]
    fn resurrects_a_tombstoned_event_instead_of_allocating_a_new_id() {
        let result = sync_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "existing-1".to_string()
            })],
            &[existing_event(|event| {
                event.deleted_at = Some("2024-01-10T00:00:00Z".to_string())
            })],
            &no_participants(),
            FROM,
            TO,
        );
        assert_eq!(
            result
                .to_update
                .iter()
                .map(|event| event.id.as_str())
                .collect::<Vec<_>>(),
            ["event-1"]
        );
        assert!(result.to_add.is_empty());
        assert!(result.to_delete.is_empty());
    }

    #[test]
    fn keeps_one_durable_row_when_duplicate_active_events_exist() {
        let result = sync_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "existing-1".to_string()
            })],
            &[
                existing_event(|_| {}),
                existing_event(|event| event.id = "event-duplicate".to_string()),
            ],
            &no_participants(),
            FROM,
            TO,
        );
        assert_eq!(
            result
                .to_update
                .iter()
                .map(|event| event.id.as_str())
                .collect::<Vec<_>>(),
            ["event-1"]
        );
        assert_eq!(result.to_delete, ["event-duplicate"]);
        assert!(result.to_add.is_empty());
    }

    #[test]
    fn only_deletes_events_from_removed_calendar() {
        let mut tracking_to_id = IndexMap::new();
        tracking_to_id.insert("tracking-cal-1".to_string(), "cal-1".to_string());
        tracking_to_id.insert("tracking-cal-2".to_string(), "cal-2".to_string());
        let ctx = SyncCtx {
            calendar_ids: vec!["cal-1".to_string(), "cal-2".to_string()],
            tracking_to_id,
        };
        let result = sync_events(
            &ctx,
            &[incoming_event(|event| {
                event.tracking_id_event = "track-2".to_string();
                event.tracking_id_calendar = "tracking-cal-2".to_string();
            })],
            &[
                existing_event(|event| {
                    event.id = "event-1".to_string();
                    event.calendar_id = "cal-1".to_string();
                    event.tracking_id_event = "track-1".to_string();
                }),
                existing_event(|event| {
                    event.id = "event-2".to_string();
                    event.calendar_id = "cal-2".to_string();
                    event.tracking_id_event = "track-2".to_string();
                }),
            ],
            &no_participants(),
            FROM,
            TO,
        );
        assert!(result.to_delete.contains(&"event-1".to_string()));
        assert!(!result.to_delete.contains(&"event-2".to_string()));
        assert_eq!(result.to_update.len(), 1);
    }

    #[test]
    fn attaches_incoming_participants_to_added_and_updated_events() {
        let alice = IncomingEventParticipant {
            email: Some("alice@example.com".to_string()),
            name: Some("Alice".to_string()),
            is_organizer: true,
            is_current_user: false,
        };
        let bob = IncomingEventParticipant {
            email: Some("bob@example.com".to_string()),
            name: Some("Bob".to_string()),
            is_organizer: false,
            is_current_user: false,
        };
        let mut participants = IndexMap::new();
        participants.insert("existing-1".to_string(), vec![alice.clone()]);
        participants.insert("incoming-1".to_string(), vec![bob.clone()]);
        let result = sync_events(
            &ctx(),
            &[
                incoming_event(|event| event.tracking_id_event = "existing-1".to_string()),
                incoming_event(|event| event.tracking_id_event = "incoming-1".to_string()),
                incoming_event(|event| event.tracking_id_event = "incoming-2".to_string()),
            ],
            &[existing_event(|_| {})],
            &participants,
            FROM,
            TO,
        );
        assert_eq!(result.to_update[0].participants, vec![alice]);
        assert_eq!(
            result
                .to_add
                .iter()
                .map(|event| (event.tracking_id_event.as_str(), event.participants.clone()))
                .collect::<Vec<_>>(),
            vec![("incoming-1", vec![bob]), ("incoming-2", vec![])]
        );
    }

    #[test]
    fn matches_participants_by_tracking_id_event_for_recurring_events() {
        let alice = IncomingEventParticipant {
            email: Some("alice@example.com".to_string()),
            name: Some("Alice".to_string()),
            is_organizer: false,
            is_current_user: false,
        };
        let mut participants = IndexMap::new();
        participants.insert("recurring-1".to_string(), vec![alice.clone()]);
        let result = sync_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "recurring-1".to_string();
                event.has_recurrence_rules = true;
                event.started_at = Some("2024-01-15T10:00:00Z".to_string());
            })],
            &[],
            &participants,
            FROM,
            TO,
        );
        assert_eq!(result.to_add.len(), 1);
        assert_eq!(result.to_add[0].participants, vec![alice]);
    }

    // syncSessionEmbeddedEvents ports

    fn session_row(id: &str, tracking_id: &str, calendar_id: &str) -> anlg_db_app::SessionSyncRow {
        anlg_db_app::SessionSyncRow {
            id: id.to_string(),
            owner_user_id: "user-1".to_string(),
            event_json: "{}".to_string(),
            tracking_id: tracking_id.to_string(),
            calendar_id: calendar_id.to_string(),
        }
    }

    #[test]
    fn updates_every_note_linked_through_a_legacy_occurrence_id() {
        let incoming = vec![incoming_event(|event| {
            event.tracking_id_event = "canonical".to_string();
            event.legacy_tracking_ids = vec!["old-master".to_string(), "old-detached".to_string()];
        })];
        let sessions = [
            session_row("one", "old-master", "cal-1"),
            session_row("two", "old-detached", "cal-1"),
            session_row("other", "old-detached", "other-calendar"),
        ];
        let updates = sync_session_embedded_events(&ctx(), &incoming, &sessions);
        assert_eq!(
            updates
                .iter()
                .map(|update| (update.session_id.as_str(), update.tracking_id.as_str()))
                .collect::<Vec<_>>(),
            [("one", "canonical"), ("two", "canonical")]
        );
    }

    #[test]
    fn does_not_rewrite_note_snapshots_for_canceled_events() {
        let updates = sync_session_embedded_events(
            &ctx(),
            &[incoming_event(|event| event.is_cancelled = true)],
            &[session_row("one", "track-1", "cal-1")],
        );
        // matching by canonical id: session tracking "track-1" vs incoming "incoming-1"
        // adjust: give matching tracking id
        let _ = updates;
        let updates = sync_session_embedded_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "track-1".to_string();
                event.is_cancelled = true;
            })],
            &[session_row("one", "track-1", "cal-1")],
        );
        assert!(updates.is_empty());
    }

    #[test]
    fn builds_an_update_for_a_matching_event_with_its_canonical_calendar_id() {
        let mut tracking_to_id = IndexMap::new();
        tracking_to_id.insert("tracking-cal-new".to_string(), "cal-new".to_string());
        let ctx = SyncCtx {
            calendar_ids: vec!["cal-new".to_string()],
            tracking_to_id,
        };
        let updates = sync_session_embedded_events(
            &ctx,
            &[incoming_event(|event| {
                event.tracking_id_event = "track-1".to_string();
                event.tracking_id_calendar = "tracking-cal-new".to_string();
            })],
            &[session_row("session-1", "track-1", "")],
        );
        assert_eq!(updates.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(&updates[0].event_json).unwrap();
        assert_eq!(parsed["title"], "Test Event");
        assert_eq!(parsed["tracking_id"], "track-1");
        assert_eq!(parsed["calendar_id"], "cal-new");
    }

    #[test]
    fn matches_recurring_events_by_occurrence_tracking_id() {
        let jan15 = session_row("session-jan15", "recurring-1:2024-01-15", "cal-1");
        let jan22 = session_row("session-jan22", "recurring-1:2024-01-22", "cal-1");
        let updates = sync_session_embedded_events(
            &ctx(),
            &[incoming_event(|event| {
                event.tracking_id_event = "recurring-1:2024-01-15".to_string();
                event.has_recurrence_rules = true;
                event.title = Some("Updated Jan 15".to_string());
            })],
            &[jan15, jan22],
        );
        assert_eq!(
            updates
                .iter()
                .map(|update| update.session_id.as_str())
                .collect::<Vec<_>>(),
            ["session-jan15"]
        );
    }

    #[test]
    fn skips_sessions_without_a_matching_event() {
        let updates = sync_session_embedded_events(
            &ctx(),
            &[incoming_event(|_| {})],
            &[session_row("session-1", "other-event", "")],
        );
        assert!(updates.is_empty());
    }

    // Real-SQLite integration tests

    async fn test_db() -> Db {
        let db = Db::connect_memory_plain().await.unwrap();
        anlg_db_app::prepare_schema(&db).await.unwrap();
        db
    }

    async fn seed_apple_db() -> Db {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at, ended_at,
                location, meeting_link, description, note, recurrence_series_id,
                has_recurrence_rules, is_all_day, provider, participants_json,
                created_at, updated_at, deleted_at)
             VALUES
                ('row-original', 'store:uid:2026-09-15', 'calendar', 'Original meeting',
                 '2026-09-15T10:00:00Z', '2026-09-15T11:00:00Z', '', '', '', '', 'uid',
                 1, 0, 'apple', '[]', '2026-09-01', '2026-09-01', NULL),
                ('row-duplicate', 'store:uid/RID=811159200', 'calendar', 'Original meeting',
                 '2026-09-15T10:00:00Z', '2026-09-15T11:00:00Z', '', '', '', '', 'uid',
                 1, 0, 'apple', '[]', '2026-09-01', '2026-09-01', NULL)",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO sessions (id, owner_user_id, event_id, event_json)
             VALUES ('note-one', 'user', 'row-original', '{}'),
                    ('note-two', 'user', 'row-duplicate', '{}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO app_settings (id, value_json, updated_at) VALUES ('ignored_events', ?, 'before')")
            .bind(
                json!([
                    {"tracking_id": "store:uid:2026-09-15", "last_seen": "2026-09-15"},
                    {"tracking_id": "other-event", "last_seen": "2026-09-15"}
                ])
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();
        db
    }

    fn apple_request(events: Vec<IncomingCalendarEvent>) -> SyncCalendarConnectionEventsRequest {
        SyncCalendarConnectionEventsRequest {
            provider: CalendarProviderType::Apple,
            connection_id: "apple".to_string(),
            from: "2026-09-14T00:00:00Z".to_string(),
            to: "2026-09-20T00:00:00Z".to_string(),
            calendars: vec![SyncCalendarRef {
                id: "calendar".to_string(),
                tracking_id_calendar: "native-calendar".to_string(),
            }],
            events,
            participants: vec![],
        }
    }

    fn apple_occurrence() -> IncomingCalendarEvent {
        incoming_event(|event| {
            event.tracking_id_event = "apple:canonical-occurrence".to_string();
            event.tracking_id_calendar = "native-calendar".to_string();
            event.legacy_tracking_ids = vec![
                "store:uid:2026-09-15".to_string(),
                "store:uid/RID=811159200".to_string(),
            ];
            event.title = Some("Rescheduled meeting".to_string());
            event.started_at = Some("2026-09-17T10:00:00Z".to_string());
            event.ended_at = Some("2026-09-17T11:00:00Z".to_string());
            event.recurrence_series_id = Some("uid".to_string());
        })
    }

    async fn ignored_setting(db: &Db) -> serde_json::Value {
        let value: String =
            sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = 'ignored_events'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        serde_json::from_str(&value).unwrap()
    }

    #[tokio::test]
    async fn retains_a_linked_row_repairs_notes_and_ignores_and_converges() {
        let db = seed_apple_db().await;
        // A linked row wins even when an unlinked duplicate is older.
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at, ended_at,
                recurrence_series_id, has_recurrence_rules, is_all_day, provider, created_at, updated_at)
             VALUES ('older-unlinked', 'store:uid:2026-09-15', 'calendar', 'Original meeting',
                '2026-09-15T10:00:00Z', '2026-09-15T11:00:00Z', 'uid', 1, 0, 'apple',
                '2026-08-01', '2026-08-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let events = vec![apple_occurrence()];
        sync_calendar_connection_events(db.pool(), apple_request(events.clone()))
            .await
            .unwrap();

        let alive: Vec<String> =
            sqlx::query_scalar("SELECT id FROM events WHERE deleted_at IS NULL ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(alive.len(), 1);
        let kept = alive[0].clone();
        assert!(["row-original", "row-duplicate"].contains(&kept.as_str()));

        let notes: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT event_id, external_event_id, event_json FROM sessions ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(notes.len(), 2);
        for (event_id, external_id, event_json) in &notes {
            assert_eq!(event_id, &kept);
            assert_eq!(external_id, "apple:canonical-occurrence");
            let parsed: serde_json::Value = serde_json::from_str(event_json).unwrap();
            assert_eq!(parsed["title"], "Rescheduled meeting");
        }

        let ignored = ignored_setting(&db).await;
        let ids: Vec<&str> = ignored
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|entry| entry["tracking_id"].as_str())
            .collect();
        assert_eq!(ids, ["apple:canonical-occurrence", "other-event"]);

        sync_calendar_connection_events(db.pool(), apple_request(events))
            .await
            .unwrap();
        let alive_again: Vec<String> =
            sqlx::query_scalar("SELECT id FROM events WHERE deleted_at IS NULL ORDER BY id")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(alive_again, [kept]);
    }

    #[tokio::test]
    async fn deduplicates_ignored_aliases_and_keeps_the_newest_last_seen() {
        let db = seed_apple_db().await;
        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = 'ignored_events'")
            .bind(
                json!([
                    {"tracking_id": "apple:canonical-occurrence", "last_seen": "2026-09-14"},
                    {"tracking_id": "store:uid:2026-09-15", "last_seen": "2026-09-16"},
                    {"tracking_id": "store:uid/RID=811159200", "last_seen": "2026-09-15"},
                    {"tracking_id": "other-event", "last_seen": "2026-09-15"}
                ])
                .to_string(),
            )
            .execute(db.pool())
            .await
            .unwrap();

        let events = vec![apple_occurrence()];
        sync_calendar_connection_events(db.pool(), apple_request(events.clone()))
            .await
            .unwrap();
        let expected = json!([
            {"tracking_id": "apple:canonical-occurrence", "last_seen": "2026-09-16"},
            {"tracking_id": "other-event", "last_seen": "2026-09-15"}
        ]);
        assert_eq!(ignored_setting(&db).await, expected);
        sync_calendar_connection_events(db.pool(), apple_request(events))
            .await
            .unwrap();
        assert_eq!(ignored_setting(&db).await, expected);
    }

    #[tokio::test]
    async fn preserves_ignored_entries_without_tracking_ids_during_alias_migration() {
        let db = seed_apple_db().await;
        let unidentified = json!([
            {"last_seen": "2026-09-14", "title": "Missing ID"},
            {"last_seen": "2026-09-15", "title": "Another missing ID"},
            {"tracking_id": null, "last_seen": "2026-09-16"},
            {"tracking_id": null, "last_seen": "2026-09-17"}
        ]);
        let mut merged = unidentified.as_array().unwrap().clone();
        merged.extend(
            ignored_setting(&db)
                .await
                .as_array()
                .unwrap()
                .iter()
                .cloned(),
        );
        sqlx::query("UPDATE app_settings SET value_json = ? WHERE id = 'ignored_events'")
            .bind(serde_json::to_string(&merged).unwrap())
            .execute(db.pool())
            .await
            .unwrap();

        sync_calendar_connection_events(db.pool(), apple_request(vec![apple_occurrence()]))
            .await
            .unwrap();
        let mut expected = unidentified.as_array().unwrap().clone();
        expected
            .push(json!({"tracking_id": "apple:canonical-occurrence", "last_seen": "2026-09-15"}));
        expected.push(json!({"tracking_id": "other-event", "last_seen": "2026-09-15"}));
        assert_eq!(
            ignored_setting(&db).await,
            serde_json::Value::Array(expected)
        );
    }

    #[tokio::test]
    async fn rolls_back_event_note_and_ignore_changes_together() {
        let db = seed_apple_db().await;
        sqlx::query(
            "CREATE TRIGGER reject_note BEFORE UPDATE ON sessions
             BEGIN SELECT RAISE(ABORT, 'test failure'); END",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let error =
            sync_calendar_connection_events(db.pool(), apple_request(vec![apple_occurrence()]))
                .await
                .unwrap_err();
        assert!(error.contains("test failure"), "{error}");

        let alive: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(alive, 2);
        assert_eq!(
            ignored_setting(&db).await[0]["tracking_id"],
            "store:uid:2026-09-15"
        );
        let external: String =
            sqlx::query_scalar("SELECT external_event_id FROM sessions WHERE id = 'note-one'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(external, "");
    }

    #[tokio::test]
    async fn migrates_ignored_ids_from_legacy_snapshots() {
        for legacy_id in [LEGACY_MAIN_VALUES_ID, LEGACY_SETTINGS_ID] {
            let db = seed_apple_db().await;
            let old = ignored_setting(&db).await;
            sqlx::query("DELETE FROM app_settings")
                .execute(db.pool())
                .await
                .unwrap();
            sqlx::query(
                "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, 'before')",
            )
            .bind(legacy_id)
            .bind(json!({ "ignored_events": old.to_string() }).to_string())
            .execute(db.pool())
            .await
            .unwrap();

            sync_calendar_connection_events(db.pool(), apple_request(vec![apple_occurrence()]))
                .await
                .unwrap();

            assert_eq!(
                ignored_setting(&db).await[0]["tracking_id"],
                "apple:canonical-occurrence"
            );
            let legacy: String =
                sqlx::query_scalar("SELECT value_json FROM app_settings WHERE id = ?")
                    .bind(legacy_id)
                    .fetch_one(db.pool())
                    .await
                    .unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&legacy).unwrap();
            assert_eq!(parsed["ignored_events"], old.to_string());
        }
    }

    #[tokio::test]
    async fn an_explicitly_empty_ignore_list_overrides_legacy_entries() {
        let db = seed_apple_db().await;
        sqlx::query(
            "INSERT INTO app_settings (id, value_json, updated_at) VALUES (?, ?, 'before')",
        )
        .bind(LEGACY_SETTINGS_ID)
        .bind(json!({ "ignored_events": ignored_setting(&db).await }).to_string())
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE app_settings SET value_json = '[]' WHERE id = 'ignored_events'")
            .execute(db.pool())
            .await
            .unwrap();

        sync_calendar_connection_events(db.pool(), apple_request(vec![apple_occurrence()]))
            .await
            .unwrap();
        assert_eq!(ignored_setting(&db).await, json!([]));
    }

    #[tokio::test]
    async fn resurrects_a_linked_legacy_tombstone_outside_the_visible_window() {
        let db = seed_apple_db().await;
        sqlx::query(
            "UPDATE events SET deleted_at = '2026-09-10',
                started_at = '2026-08-15T10:00:00Z', ended_at = '2026-08-15T11:00:00Z'",
        )
        .execute(db.pool())
        .await
        .unwrap();

        sync_calendar_connection_events(db.pool(), apple_request(vec![apple_occurrence()]))
            .await
            .unwrap();

        let alive: i64 = sqlx::query_scalar("SELECT count(*) FROM events WHERE deleted_at IS NULL")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(alive, 1);
        let distinct: i64 = sqlx::query_scalar("SELECT count(DISTINCT event_id) FROM sessions")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(distinct, 1);
    }

    async fn seed_participant_db() -> Db {
        let db = test_db().await;
        sqlx::query(
            "INSERT INTO sessions (id, owner_user_id, event_json)
             VALUES ('note-1', 'owner', '{\"tracking_id\": \"tracking-1\"}')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO humans (id, name, email, created_at, updated_at)
             VALUES ('owner', '', 'owner@acme.com', '2026-09-01', '2026-09-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        db
    }

    fn participant(email: &str, name: Option<&str>) -> IncomingEventParticipant {
        IncomingEventParticipant {
            name: name.map(str::to_string),
            email: Some(email.to_string()),
            is_organizer: false,
            is_current_user: false,
        }
    }

    fn google_request(
        events: Vec<IncomingCalendarEvent>,
        participants: Vec<IncomingEventParticipants>,
    ) -> SyncCalendarConnectionEventsRequest {
        SyncCalendarConnectionEventsRequest {
            provider: CalendarProviderType::Google,
            connection_id: "conn".to_string(),
            from: "2026-09-14T00:00:00Z".to_string(),
            to: "2026-09-20T00:00:00Z".to_string(),
            calendars: vec![SyncCalendarRef {
                id: "calendar".to_string(),
                tracking_id_calendar: "primary".to_string(),
            }],
            events,
            participants,
        }
    }

    fn google_event() -> IncomingCalendarEvent {
        incoming_event(|event| {
            event.tracking_id_event = "tracking-1".to_string();
            event.tracking_id_calendar = "primary".to_string();
            event.started_at = Some("2026-09-16T10:00:00Z".to_string());
            event.ended_at = Some("2026-09-16T11:00:00Z".to_string());
        })
    }

    #[tokio::test]
    async fn persists_attendance_for_inserted_and_updated_events() {
        let db = seed_participant_db().await;
        let initial_attendance = r#"{"version":1,"self_status":"accepted"}"#;
        let updated_attendance = r#"{"version":1,"self_status":"declined"}"#;
        let mut event = google_event();
        event.attendance_json = Some(initial_attendance.to_string());

        sync_calendar_connection_events(db.pool(), google_request(vec![event.clone()], vec![]))
            .await
            .unwrap();
        let stored: Option<String> =
            sqlx::query_scalar("SELECT attendance_json FROM events WHERE tracking_id_event = ?")
                .bind(&event.tracking_id_event)
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(stored.as_deref(), Some(initial_attendance));

        event.attendance_json = Some(updated_attendance.to_string());
        sync_calendar_connection_events(db.pool(), google_request(vec![event], vec![]))
            .await
            .unwrap();
        let stored: Option<String> =
            sqlx::query_scalar("SELECT attendance_json FROM events WHERE tracking_id_event = ?")
                .bind("tracking-1")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(stored.as_deref(), Some(updated_attendance));
    }

    async fn humans(db: &Db) -> Vec<(String, String, String)> {
        sqlx::query_as(
            "SELECT h.email, h.name, COALESCE(o.name, '') AS company
             FROM humans h LEFT JOIN organizations o ON o.id = h.organization_id
             WHERE h.deleted_at IS NULL ORDER BY h.email",
        )
        .fetch_all(db.pool())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn creates_named_humans_linked_to_inferred_organizations_and_converges() {
        let db = seed_participant_db().await;
        let participants = vec![IncomingEventParticipants {
            tracking_id_event: "tracking-1".to_string(),
            participants: vec![
                participant("simon.goldstein@ionprotocol.io", None),
                participant("jane.doe@gmail.com", None),
                participant("owner@acme.com", None),
            ],
        }];
        let request = google_request(vec![google_event()], participants.clone());

        sync_calendar_connection_events(db.pool(), request.clone())
            .await
            .unwrap();
        assert_eq!(
            humans(&db).await,
            [
                (
                    "jane.doe@gmail.com".to_string(),
                    "Jane Doe".to_string(),
                    String::new()
                ),
                ("owner@acme.com".to_string(), String::new(), String::new()),
                (
                    "simon.goldstein@ionprotocol.io".to_string(),
                    "Simon Goldstein".to_string(),
                    "Ionprotocol".to_string()
                ),
            ]
        );
        let display_names: Vec<String> =
            sqlx::query_scalar("SELECT display_name FROM session_participants ORDER BY email")
                .fetch_all(db.pool())
                .await
                .unwrap();
        assert_eq!(display_names, ["Jane Doe", "", "Simon Goldstein"]);

        sync_calendar_connection_events(db.pool(), request)
            .await
            .unwrap();
        assert_eq!(humans(&db).await.len(), 3);
        let orgs: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(orgs, 1);
    }

    #[tokio::test]
    async fn enriches_email_named_humans_reuses_organizations_and_keeps_user_edits() {
        let db = seed_participant_db().await;
        sqlx::query(
            "INSERT INTO organizations (id, name, created_at, updated_at)
             VALUES ('org-acme', 'Acme', '2026-09-01', '2026-09-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO humans (id, name, email, created_at, updated_at) VALUES
             ('h-alice', 'alice@acme.com', 'alice@acme.com', '2026-09-01', '2026-09-01'),
             ('h-bob', 'Robert Builder', 'bob@acme.com', '2026-09-01', '2026-09-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        sync_calendar_connection_events(
            db.pool(),
            google_request(
                vec![google_event()],
                vec![IncomingEventParticipants {
                    tracking_id_event: "tracking-1".to_string(),
                    participants: vec![
                        participant("alice@acme.com", None),
                        participant("bob@acme.com", Some("Bob")),
                    ],
                }],
            ),
        )
        .await
        .unwrap();

        assert_eq!(
            humans(&db).await,
            [
                (
                    "alice@acme.com".to_string(),
                    "Alice".to_string(),
                    "Acme".to_string()
                ),
                (
                    "bob@acme.com".to_string(),
                    "Robert Builder".to_string(),
                    "Acme".to_string()
                ),
                ("owner@acme.com".to_string(), String::new(), String::new()),
            ]
        );
        let orgs: i64 = sqlx::query_scalar("SELECT count(*) FROM organizations")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(orgs, 1);
    }

    #[tokio::test]
    async fn keeps_human_names_containing_at_and_user_edited_names() {
        let db = seed_participant_db().await;
        sqlx::query(
            "INSERT INTO organizations (id, name, created_at, updated_at)
             VALUES ('org-consulting', 'Acme Consulting', '2026-09-01', '2026-09-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO humans (id, name, email, organization_id, created_at, updated_at) VALUES
             ('h-jane', 'Jane @ Acme', 'jane@acme.com', 'org-consulting', '2026-09-01', '2026-09-01'),
             ('h-alice', 'alice@acme.com', 'alice@acme.com', 'org-consulting', '2026-09-01', '2026-09-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        sync_calendar_connection_events(
            db.pool(),
            google_request(
                vec![google_event()],
                vec![IncomingEventParticipants {
                    tracking_id_event: "tracking-1".to_string(),
                    participants: vec![
                        participant("jane@acme.com", None),
                        participant("alice@acme.com", None),
                    ],
                }],
            ),
        )
        .await
        .unwrap();

        assert_eq!(
            humans(&db).await,
            [
                (
                    "alice@acme.com".to_string(),
                    "Alice".to_string(),
                    "Acme Consulting".to_string()
                ),
                (
                    "jane@acme.com".to_string(),
                    "Jane @ Acme".to_string(),
                    "Acme Consulting".to_string()
                ),
                ("owner@acme.com".to_string(), String::new(), String::new()),
            ]
        );
    }

    #[tokio::test]
    async fn commits_event_session_human_and_participant_writes_together() {
        let db = seed_participant_db().await;

        sync_calendar_connection_events(
            db.pool(),
            google_request(
                vec![google_event()],
                vec![IncomingEventParticipants {
                    tracking_id_event: "tracking-1".to_string(),
                    participants: vec![participant("alice@example.com", Some("Alice"))],
                }],
            ),
        )
        .await
        .unwrap();

        let event: (String, String, String) = sqlx::query_as(
            "SELECT tracking_id_event, calendar_id, provider FROM events WHERE deleted_at IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            event,
            (
                "tracking-1".to_string(),
                "calendar".to_string(),
                "google".to_string()
            )
        );

        let session: (String, String) = sqlx::query_as(
            "SELECT external_event_id, external_provider FROM sessions WHERE id = 'note-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(session, ("tracking-1".to_string(), "google".to_string()));

        let participants: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM session_participants WHERE source = 'auto' AND deleted_at IS NULL",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(participants, 1);
    }

    #[tokio::test]
    async fn normalizes_missing_optional_fields_when_updating_events() {
        let db = seed_participant_db().await;
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at, ended_at,
                location, meeting_link, description, recurrence_series_id, provider,
                created_at, updated_at)
             VALUES ('event-1', 'tracking-1', 'calendar', 'Meeting', '2026-09-16T10:00:00Z',
                '2026-09-16T11:00:00Z', 'Room 1', 'https://meet.example.com/room',
                'Description', 'series-1', 'google', '2026-01-01', '2026-01-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        sync_calendar_connection_events(
            db.pool(),
            google_request(
                vec![incoming_event(|event| {
                    event.tracking_id_event = "tracking-1".to_string();
                    event.tracking_id_calendar = "primary".to_string();
                    event.title = Some("Updated meeting".to_string());
                    event.started_at = Some("2026-09-16T10:00:00Z".to_string());
                    event.ended_at = Some("2026-09-16T11:00:00Z".to_string());
                })],
                vec![],
            ),
        )
        .await
        .unwrap();

        let row: (String, String, String, String) = sqlx::query_as(
            "SELECT location, meeting_link, description, recurrence_series_id
             FROM events WHERE id = 'event-1'",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(
            row,
            (String::new(), String::new(), String::new(), String::new())
        );
        let title: String = sqlx::query_scalar("SELECT title FROM events WHERE id = 'event-1'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(title, "Updated meeting");
    }

    #[tokio::test]
    async fn tombstones_events_whose_timestamps_use_a_space_separator() {
        let db = seed_participant_db().await;
        sqlx::query(
            "INSERT INTO events (id, tracking_id_event, calendar_id, title, started_at, ended_at,
                provider, created_at, updated_at)
             VALUES ('event-1', 'tracking-1', 'calendar', 'Meeting', '2026-09-16 10:00:00',
                '2026-09-16 11:00:00', 'google', '2026-01-01', '2026-01-01')",
        )
        .execute(db.pool())
        .await
        .unwrap();

        let mut request = google_request(vec![], vec![]);
        request.from = "2026-09-14T00:00:00.000Z".to_string();
        request.to = "2026-09-21T00:00:00.000Z".to_string();
        sync_calendar_connection_events(db.pool(), request)
            .await
            .unwrap();

        let deleted_at: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM events WHERE id = 'event-1'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert!(deleted_at.is_some());
    }
}
