use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use specta::Type;
use sqlx::SqlitePool;

use crate::transaction_utils::js_iso8601_timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CreateSessionRequest {
    pub title: String,
    pub user_id: String,
    pub event_json: String,
    pub folder_path: String,
    pub raw_md: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EventParticipantIdentity {
    pub email: String,
    pub name: String,
    pub company_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct CreateEventSessionRequest {
    pub event_id: String,
    pub user_id: String,
    pub title: Option<String>,
    pub participants: Vec<EventParticipantIdentity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EventSessionResult {
    pub session_id: String,
    pub created: bool,
}

#[derive(Serialize)]
struct SessionEvent<'a> {
    tracking_id: &'a str,
    calendar_id: &'a str,
    title: &'a str,
    started_at: &'a str,
    ended_at: &'a str,
    is_all_day: bool,
    has_recurrence_rules: bool,
    location: Option<&'a str>,
    meeting_link: Option<&'a str>,
    description: Option<&'a str>,
    recurrence_series_id: Option<&'a str>,
}

pub async fn create_session(
    pool: &SqlitePool,
    request: CreateSessionRequest,
) -> Result<String, String> {
    let session_id = uuid::Uuid::new_v4().to_string();
    let participant_id = uuid::Uuid::new_v4().to_string();
    let now = js_iso8601_timestamp();
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    anlg_db_app::insert_session(
        &mut transaction,
        &session_id,
        &request.title,
        &request.user_id,
        &request.event_json,
        &request.folder_path,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;
    anlg_db_app::insert_empty_session_note(&mut transaction, &session_id, &request.raw_md, &now)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::upsert_session_owner_human(&mut transaction, &session_id, &now)
        .await
        .map_err(|error| error.to_string())?;
    anlg_db_app::insert_owner_session_participant(
        &mut transaction,
        &participant_id,
        &session_id,
        &now,
    )
    .await
    .map_err(|error| error.to_string())?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    Ok(session_id)
}

pub async fn create_session_for_event(
    pool: &SqlitePool,
    request: CreateEventSessionRequest,
) -> Result<Option<EventSessionResult>, String> {
    let mut transaction = pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|error| error.to_string())?;

    let Some(event) = anlg_db_app::load_event_session_row(&mut transaction, &request.event_id)
        .await
        .map_err(|error| error.to_string())?
    else {
        transaction
            .rollback()
            .await
            .map_err(|error| error.to_string())?;
        return Ok(None);
    };

    if let Some(existing_session_id) = anlg_db_app::find_session_id_for_event(
        &mut transaction,
        &event.id,
        &event.tracking_id_event,
        "",
    )
    .await
    .map_err(|error| error.to_string())?
    {
        let now = js_iso8601_timestamp();
        anlg_db_app::relink_session_to_event(
            &mut transaction,
            &existing_session_id,
            &event.id,
            &now,
        )
        .await
        .map_err(|error| error.to_string())?;
        run_event_participants(
            &mut transaction,
            &existing_session_id,
            &request.participants,
            &now,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        return Ok(Some(EventSessionResult {
            session_id: existing_session_id,
            created: false,
        }));
    }

    let session_id = uuid::Uuid::new_v4().to_string();
    let now = js_iso8601_timestamp();
    let session_event = SessionEvent {
        tracking_id: &event.tracking_id_event,
        calendar_id: &event.calendar_id,
        title: &event.title,
        started_at: &event.started_at,
        ended_at: &event.ended_at,
        is_all_day: event.is_all_day != 0,
        has_recurrence_rules: event.has_recurrence_rules != 0,
        location: event.location.as_deref(),
        meeting_link: event.meeting_link.as_deref(),
        description: event.description.as_deref(),
        recurrence_series_id: event.recurrence_series_id.as_deref(),
    };
    let event_json = serde_json::to_string(&session_event).map_err(|error| error.to_string())?;
    let title = request.title.clone().unwrap_or_else(|| event.title.clone());

    let inserted = anlg_db_app::insert_event_session(
        &mut transaction,
        &session_id,
        &request.user_id,
        &title,
        &now,
        &event,
        &event_json,
    )
    .await
    .map_err(|error| error.to_string())?;
    anlg_db_app::insert_empty_session_note(&mut transaction, &session_id, "", &now)
        .await
        .map_err(|error| error.to_string())?;
    run_event_participants(&mut transaction, &session_id, &request.participants, &now).await?;

    let created_session_id = anlg_db_app::find_session_id_for_event(
        &mut transaction,
        &event.id,
        &event.tracking_id_event,
        &session_id,
    )
    .await
    .map_err(|error| error.to_string())?
    .ok_or_else(|| format!("Failed to create a session for event {}", request.event_id))?;

    transaction
        .commit()
        .await
        .map_err(|error| error.to_string())?;

    Ok(Some(EventSessionResult {
        session_id: created_session_id,
        created: inserted == 1,
    }))
}

async fn run_event_participants(
    conn: &mut sqlx::SqliteConnection,
    session_id: &str,
    participants: &[EventParticipantIdentity],
    now: &str,
) -> Result<(), String> {
    let mut seen = HashSet::new();
    let emails: Vec<String> = participants
        .iter()
        .map(|participant| participant.email.to_lowercase())
        .filter(|email| seen.insert(email.clone()))
        .collect();
    let mut humans_by_email: HashMap<String, String> = HashMap::new();
    for row in anlg_db_app::find_human_ids_by_email(&mut *conn, &emails)
        .await
        .map_err(|error| error.to_string())?
    {
        humans_by_email.insert(row.email.to_lowercase(), row.id);
    }

    for participant in participants {
        let email_key = participant.email.to_lowercase();
        let known = humans_by_email.contains_key(&email_key);
        let human_id = humans_by_email
            .get(&email_key)
            .cloned()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let company_name = participant.company_name.as_deref().unwrap_or("");
        let human = anlg_db_app::EventHumanInsert {
            human_id: &human_id,
            name: &participant.name,
            email: &participant.email,
            company_name,
            now,
            session_id,
        };

        if !known {
            if let Some(company) = &participant.company_name {
                anlg_db_app::insert_new_participant_organization(
                    &mut *conn,
                    &anlg_db_app::EventParticipantInsert {
                        organization_id: &uuid::Uuid::new_v4().to_string(),
                        company_name: company,
                        now,
                        session_id,
                        human_id: &human_id,
                        email: &participant.email,
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            }
            anlg_db_app::insert_event_human(&mut *conn, &human)
                .await
                .map_err(|error| error.to_string())?;
        } else {
            if let Some(company) = &participant.company_name {
                anlg_db_app::insert_existing_participant_organization(
                    &mut *conn,
                    &anlg_db_app::EventParticipantInsert {
                        organization_id: &uuid::Uuid::new_v4().to_string(),
                        company_name: company,
                        now,
                        session_id,
                        human_id: &human_id,
                        email: &participant.email,
                    },
                )
                .await
                .map_err(|error| error.to_string())?;
            }
            anlg_db_app::update_event_human(&mut *conn, &human)
                .await
                .map_err(|error| error.to_string())?;
        }

        anlg_db_app::insert_event_session_participant(
            &mut *conn,
            &uuid::Uuid::new_v4().to_string(),
            &human,
        )
        .await
        .map_err(|error| error.to_string())?;
    }

    Ok(())
}
