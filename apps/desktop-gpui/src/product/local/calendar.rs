use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use anlg_calendar::{CalendarEvent, CalendarListItem, CalendarProviderType, EventFilter};
use anlg_calendar_interface::{AttendeeRole, EventStatus};
use anlg_db_execute::TransactionStatement as DbStatement;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle, ServiceError};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use uuid::Uuid;

use super::failure;
use crate::product::services::Scope;

pub struct AccessToken(String);

impl AccessToken {
    pub fn new(value: String) -> Self {
        Self(value)
    }
}

pub trait CalendarAuth: Send + Sync {
    fn token(&self, cancel: CancellationToken) -> BoxFuture<'static, Result<AccessToken>>;
    fn connect(
        &self,
        provider: CalendarProviderType,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>>;
    fn disconnect(
        &self,
        provider: CalendarProviderType,
        connection: String,
        cancel: CancellationToken,
    ) -> BoxFuture<'static, Result<()>>;
}

#[derive(Clone)]
pub struct CalendarService {
    pub api_base: Arc<str>,
    pub auth: Arc<dyn CalendarAuth>,
    page: Arc<Mutex<(Scope, usize)>>,
}

impl CalendarService {
    pub async fn reconcile(
        &self,
        runtime: &RuntimeHandle,
        callback: desktop_runtime::deeplink::IntegrationCallback,
        cancel: CancellationToken,
    ) -> Result<()> {
        if callback.status != "success" {
            return Err(failure("Calendar connection was not completed"));
        }
        let (provider, name) = match callback.integration_id.as_str() {
            "google-calendar" => (CalendarProviderType::Google, "google"),
            "outlook" => (CalendarProviderType::Outlook, "outlook"),
            _ => return Ok(()),
        };
        if let Some(connection) = callback.disconnected_connection_id {
            runtime.submit(move |services| async move {
                services.executor.execute_transaction(vec![
                    DbStatement { sql: "UPDATE events SET deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE calendar_id IN (SELECT id FROM calendars WHERE provider=? AND connection_id=?) AND deleted_at IS NULL".into(), params:vec![json!(name),json!(connection)], expected_rows_affected:None },
                    DbStatement { sql: "UPDATE calendars SET enabled=0,deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE provider=? AND connection_id=?".into(), params:vec![json!(name),json!(connection)], expected_rows_affected:None },
                ]).await.map_err(failure)?;
                Ok(())
            })?.receive().await?;
        } else {
            for connection in self
                .connections(cancel.clone())
                .await?
                .into_iter()
                .filter(|entry| entry.provider == provider)
                .flat_map(|entry| entry.connection_ids)
            {
                for calendar in self
                    .discover(runtime, provider, connection, cancel.clone())
                    .await?
                {
                    if calendar["enabled"].as_bool() == Some(true) {
                        self.refresh(
                            runtime,
                            calendar["id"]
                                .as_str()
                                .ok_or(ServiceError::Conflict)?
                                .into(),
                            cancel.clone(),
                        )
                        .await?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn new(api_base: Arc<str>, auth: Arc<dyn CalendarAuth>) -> Self {
        Self {
            api_base,
            auth,
            page: Arc::new(Mutex::new((Scope::default(), 0))),
        }
    }

    pub fn page(&self, scope: &Scope) -> Result<usize> {
        let mut state = self.page.lock().map_err(|_| ServiceError::Closed)?;
        if &state.0 != scope {
            *state = (scope.clone(), 0);
        }
        Ok(state.1)
    }

    pub fn set_page(&self, scope: Scope, offset: usize) -> Result<()> {
        *self.page.lock().map_err(|_| ServiceError::Closed)? = (scope, offset);
        Ok(())
    }

    pub async fn discover(
        &self,
        runtime: &RuntimeHandle,
        provider: CalendarProviderType,
        connection: String,
        cancel: CancellationToken,
    ) -> Result<Vec<Value>> {
        let calendars = self
            .calendars(provider, &connection, cancel.clone())
            .await?;
        runtime.submit(move |services| async move {
            let provider=super::provider_id(provider);
            let mut rows=Vec::new();
            let mut statements=Vec::new();
            let ids=serde_json::to_string(&calendars.iter().map(|calendar|&calendar.id).collect::<Vec<_>>()).map_err(failure)?;
            statements.push(DbStatement{expected_rows_affected:None,sql:"UPDATE calendars SET deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE provider=? AND connection_id=? AND deleted_at IS NULL AND tracking_id_calendar NOT IN (SELECT value FROM json_each(?))".into(),params:vec![json!(provider),json!(connection),json!(ids)]});
            statements.push(DbStatement{expected_rows_affected:None,sql:"UPDATE events SET deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE calendar_id IN (SELECT id FROM calendars WHERE provider=? AND connection_id=? AND deleted_at IS NOT NULL) AND deleted_at IS NULL".into(),params:vec![json!(provider),json!(connection)]});
            for calendar in calendars {
                if cancel.is_cancelled() {return Err(ServiceError::Cancelled);}
                let existing=services.executor.execute("SELECT * FROM calendars WHERE provider=? AND connection_id=? AND tracking_id_calendar=?".into(),vec![json!(provider),json!(connection),json!(calendar.id)]).await.map_err(failure)?;
                if existing.len()>1 {return Err(ServiceError::Conflict);}
                let id=existing.first().and_then(|row|row["id"].as_str()).map(str::to_owned).unwrap_or_else(||Uuid::new_v4().to_string());
                let enabled=existing.first().is_some_and(|row|row["deleted_at"].is_null() && row["enabled"].as_i64()==Some(1));
                statements.push(DbStatement {expected_rows_affected:None,sql:"INSERT INTO calendars (id,tracking_id_calendar,name,enabled,provider,source,color,connection_id) VALUES (?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,source=excluded.source,color=excluded.color,enabled=excluded.enabled,deleted_at=NULL,updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')".into(),params:vec![json!(id),json!(calendar.id),json!(calendar.title),json!(enabled as i64),json!(provider),json!(calendar.source.unwrap_or_default()),json!(calendar.color.unwrap_or_else(||"#888".into())),json!(connection)]});
                rows.push(json!({"id":id,"name":calendar.title,"enabled":enabled,"provider":provider,"connection":connection}));
            }
            if cancel.is_cancelled() {return Err(ServiceError::Cancelled);}
            services.executor.execute_transaction(statements).await.map_err(failure)?;
            Ok(rows)
        })?.receive().await
    }

    pub async fn toggle(
        &self,
        runtime: &RuntimeHandle,
        id: String,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.apply_selection(runtime, id, None, cancel).await
    }

    pub async fn refresh(
        &self,
        runtime: &RuntimeHandle,
        id: String,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.apply_selection(runtime, id, Some(true), cancel).await
    }

    async fn apply_selection(
        &self,
        runtime: &RuntimeHandle,
        id: String,
        desired: Option<bool>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let lookup = id.clone();
        let row = runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(
                        "SELECT * FROM calendars WHERE id=? AND deleted_at IS NULL".into(),
                        vec![json!(lookup)],
                    )
                    .await
                    .map_err(failure)?
                    .into_iter()
                    .next()
                    .ok_or(ServiceError::Conflict)
            })?
            .receive()
            .await?;
        let previous = row["enabled"].as_i64() == Some(1);
        if desired.is_some() && !previous {
            return Err(ServiceError::Conflict);
        }
        let enabled = desired.unwrap_or(!previous);
        let provider = super::parse_provider(row["provider"].as_str().unwrap_or(""))?;
        let from = chrono::Utc::now() - chrono::Duration::days(7);
        let to = from + chrono::Duration::days(37);
        let events = if enabled {
            self.events(
                provider,
                row["connection_id"].as_str().unwrap_or(""),
                EventFilter {
                    from,
                    to,
                    calendar_tracking_id: row["tracking_id_calendar"]
                        .as_str()
                        .ok_or(ServiceError::Conflict)?
                        .into(),
                },
                cancel.clone(),
            )
            .await?
        } else {
            Vec::new()
        };
        runtime.submit(move |services| async move {
            let now=chrono::Utc::now().to_rfc3339();
            let mut statements=vec![DbStatement {expected_rows_affected:Some(1),sql:"UPDATE calendars SET enabled=?,updated_at=? WHERE id=? AND deleted_at IS NULL AND enabled=?".into(),params:vec![json!(enabled as i64),json!(now),json!(id),json!(previous as i64)]}];
            if enabled {
                let identities=events.iter().flat_map(|event|std::iter::once(&event.id).chain(event.legacy_ids.iter())).collect::<Vec<_>>();
                statements.push(DbStatement{expected_rows_affected:None,sql:"UPDATE events SET deleted_at=?,updated_at=? WHERE calendar_id=? AND deleted_at IS NULL AND julianday(started_at)<=julianday(?) AND julianday(COALESCE(NULLIF(ended_at,''),started_at))>=julianday(?) AND tracking_id_event NOT IN (SELECT value FROM json_each(?))".into(),params:vec![json!(now),json!(now),json!(id),json!(to.to_rfc3339()),json!(from.to_rfc3339()),json!(serde_json::to_string(&identities).map_err(failure)?)]});
                if let Some(migration)=migrate_ignored(&events,&now)? {statements.push(migration);}
            }
            if !enabled {
                statements.push(DbStatement {expected_rows_affected:None,sql:"UPDATE events SET deleted_at=?,updated_at=? WHERE calendar_id=? AND deleted_at IS NULL".into(),params:vec![json!(now),json!(now),json!(id)]});
            }
            for event in events {
                if cancel.is_cancelled() {return Err(ServiceError::Cancelled);}
                let mut identities=event.legacy_ids;
                identities.push(event.id.clone());
                let mut existing_ids=Vec::new();
                for tracking_id in identities {
                    let found=services.executor.execute("SELECT id FROM events WHERE calendar_id=? AND tracking_id_event=?".into(),vec![json!(id),json!(tracking_id)]).await.map_err(failure)?;
                    existing_ids.extend(found.into_iter().filter_map(|row|row["id"].as_str().map(str::to_owned)));
                }
                existing_ids.sort();existing_ids.dedup();
                if existing_ids.len()>1 {return Err(ServiceError::Conflict);}
                if existing_ids.is_empty() && event.status==EventStatus::Cancelled {continue;}
                let event_id=existing_ids.pop().unwrap_or_else(||Uuid::new_v4().to_string());
                let organizer_email=event.organizer.as_ref().and_then(|person|person.email.as_ref()).map(|email|email.to_lowercase());
                let mut participants=Vec::new();
                if let Some(person)=event.organizer {participants.push(json!({"name":person.name,"email":person.email,"is_organizer":true,"is_current_user":person.is_current_user}));}
                for person in event.attendees {
                    if person.role==AttendeeRole::NonParticipant || (organizer_email.is_some() && person.email.as_ref().map(|email|email.to_lowercase())==organizer_email) {continue;}
                    participants.push(json!({"name":person.name,"email":person.email,"is_organizer":false,"is_current_user":person.is_current_user}));
                }
                let deleted=if event.status==EventStatus::Cancelled {json!(now)} else {Value::Null};
                statements.push(DbStatement {expected_rows_affected:None,sql:"INSERT INTO events (id,tracking_id_event,calendar_id,title,started_at,ended_at,location,meeting_link,description,recurrence_series_id,has_recurrence_rules,is_all_day,provider,participants_json,deleted_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET tracking_id_event=excluded.tracking_id_event,title=excluded.title,started_at=excluded.started_at,ended_at=excluded.ended_at,location=excluded.location,meeting_link=excluded.meeting_link,description=excluded.description,recurrence_series_id=excluded.recurrence_series_id,has_recurrence_rules=excluded.has_recurrence_rules,is_all_day=excluded.is_all_day,participants_json=excluded.participants_json,deleted_at=excluded.deleted_at,updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')".into(),params:vec![json!(event_id),json!(event.id),json!(id),json!(event.title),json!(event.started_at),json!(event.ended_at),json!(event.location.unwrap_or_default()),json!(event.meeting_link.unwrap_or_default()),json!(event.description.unwrap_or_default()),json!(event.recurring_event_id.unwrap_or_default()),json!(event.has_recurrence_rules as i64),json!(event.is_all_day as i64),json!(super::provider_id(provider)),json!(serde_json::to_string(&participants).map_err(failure)?),deleted]});
            }
            if cancel.is_cancelled() {return Err(ServiceError::Cancelled);}
            services.executor.execute_transaction(statements).await.map_err(failure)?;
            Ok(())
        })?.receive().await
    }
    pub async fn connections(
        &self,
        cancel: CancellationToken,
    ) -> Result<Vec<anlg_calendar::ProviderConnectionIds>> {
        if cancel.is_cancelled() {
            return Err(desktop_runtime::ServiceError::Cancelled);
        }
        let token = self.auth.token(cancel.clone()).await?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", token.0)
                .parse()
                .map_err(|_| failure("Invalid calendar authorization"))?,
        );
        let client = anlg_api_client::Client::new_with_client(
            &self.api_base,
            reqwest::Client::builder()
                .default_headers(headers)
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(failure)?,
        );
        let connections = tokio::select! {
            _ = cancel.cancelled() => return Err(desktop_runtime::ServiceError::Cancelled),
            result = client.list_connections() => result.map_err(|_|failure("Calendar connection discovery failed"))?.into_inner().connections,
        };
        Ok(
            [CalendarProviderType::Google, CalendarProviderType::Outlook]
                .into_iter()
                .map(|provider| {
                    let integration = if provider == CalendarProviderType::Google {
                        "google-calendar"
                    } else {
                        "outlook"
                    };
                    anlg_calendar::ProviderConnectionIds {
                        provider,
                        connection_ids: connections
                            .iter()
                            .filter(|connection| connection.integration_id == integration)
                            .map(|connection| connection.connection_id.clone())
                            .collect(),
                    }
                })
                .collect(),
        )
    }

    pub fn create_event(
        &self,
        provider: CalendarProviderType,
        input: anlg_calendar::CreateEventInput,
    ) -> Result<String> {
        anlg_calendar::create_event(provider, input).map_err(failure)
    }

    pub fn meeting_link(&self, text: &str) -> Option<String> {
        anlg_calendar::parse_meeting_link(text)
    }

    async fn token(
        &self,
        provider: CalendarProviderType,
        cancel: CancellationToken,
    ) -> Result<AccessToken> {
        if provider == CalendarProviderType::Apple {
            Ok(AccessToken(String::new()))
        } else {
            self.auth.token(cancel).await
        }
    }

    pub async fn calendars(
        &self,
        provider: CalendarProviderType,
        connection: &str,
        cancel: CancellationToken,
    ) -> Result<Vec<CalendarListItem>> {
        if cancel.is_cancelled() {
            return Err(desktop_runtime::ServiceError::Cancelled);
        }
        let token = self.token(provider, cancel.clone()).await?;
        tokio::select! {
            _ = cancel.cancelled() => Err(desktop_runtime::ServiceError::Cancelled),
            result = anlg_calendar::list_calendars(&self.api_base, &token.0, provider, connection) => result.map_err(failure),
        }
    }

    pub async fn events(
        &self,
        provider: CalendarProviderType,
        connection: &str,
        filter: EventFilter,
        cancel: CancellationToken,
    ) -> Result<Vec<CalendarEvent>> {
        if cancel.is_cancelled() {
            return Err(desktop_runtime::ServiceError::Cancelled);
        }
        let token = self.token(provider, cancel.clone()).await?;
        tokio::select! {
            _ = cancel.cancelled() => Err(desktop_runtime::ServiceError::Cancelled),
            result = anlg_calendar::list_events(&self.api_base, &token.0, provider, connection, filter) => result.map_err(failure),
        }
    }
}

fn migrate_ignored(events: &[CalendarEvent], now: &str) -> Result<Option<DbStatement>> {
    let mut aliases = BTreeMap::new();
    let mut ambiguous = BTreeSet::new();
    for event in events {
        for old in &event.legacy_ids {
            if old == &event.id {
                continue;
            }
            if let Some(previous) = aliases.insert(old, &event.id)
                && previous != &event.id
            {
                ambiguous.insert(old);
            }
        }
    }
    aliases.retain(|id, _| !ambiguous.contains(id));
    if aliases.is_empty() {
        return Ok(None);
    }
    Ok(Some(DbStatement {
        expected_rows_affected:None,
        sql:r#"
WITH current AS (
 SELECT COALESCE(
  (SELECT value_json FROM app_settings WHERE id='ignored_events'),
  (SELECT CASE WHEN json_valid(value_json) THEN json_extract(value_json,'$.ignored_events') END FROM app_settings WHERE id='legacy_main_values_document'),
  (SELECT CASE WHEN json_valid(value_json) THEN json_extract(value_json,'$.ignored_events') END FROM app_settings WHERE id='legacy_settings_document'),
  '[]'
 ) AS value_json
), aliases AS (
 SELECT key AS old_id,value AS new_id FROM json_each(?)
), entries AS (
 SELECT item.key AS position,item.value,aliases.new_id FROM current,
 json_each(CASE WHEN json_valid(current.value_json) THEN current.value_json ELSE '[]' END) AS item
 LEFT JOIN aliases ON aliases.old_id=json_extract(item.value,'$.tracking_id')
), normalized AS (
 SELECT position,CASE WHEN new_id IS NULL THEN value ELSE json_set(value,'$.tracking_id',new_id) END AS value FROM entries
), ranked AS (
 SELECT value,position,ROW_NUMBER() OVER (
 PARTITION BY json_extract(value,'$.tracking_id'),CASE WHEN json_extract(value,'$.tracking_id') IS NULL THEN position END
 ORDER BY json_extract(value,'$.last_seen') DESC,position) AS rank FROM normalized
)
INSERT INTO app_settings (id,value_json,updated_at)
SELECT 'ignored_events',(SELECT json_group_array(json(value)) FROM (SELECT value FROM ranked WHERE rank=1 ORDER BY position)),?
WHERE EXISTS (SELECT 1 FROM entries WHERE new_id IS NOT NULL)
ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at
"#.into(),
        params:vec![json!(serde_json::to_string(&aliases).map_err(failure)?),json!(now)]
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn legacy_event_aliases_preserve_ignore_state_and_unrecognized_fields() {
        let event: CalendarEvent = serde_json::from_value(json!({
            "provider":"apple","id":"current","legacy_ids":["legacy"],"calendar_id":"calendar",
            "external_id":"uid","title":"日本語","started_at":"2026-09-20T12:00:00Z",
            "ended_at":"2026-09-20T13:00:00Z","is_all_day":false,"status":"confirmed",
            "attendees":[],"has_recurrence_rules":false,"raw":"{}"
        }))
        .unwrap();
        let statement = migrate_ignored(std::slice::from_ref(&event), "2026-09-21T00:00:00Z")
            .unwrap()
            .unwrap();
        let root = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: root.path().join("test.sqlite"),
        })
        .unwrap();
        ready.receive().await.unwrap();
        let rows=runtime.submit(move |services|async move {
            services.executor.execute_transaction(vec![
                DbStatement{expected_rows_affected:None,sql:"INSERT INTO app_settings(id,value_json) VALUES ('legacy_main_values_document',?)".into(),params:vec![json!(json!({"ignored_events":[{"tracking_id":"legacy","last_seen":"2026-09-20","custom":"preserved"},{"tracking_id":"current","last_seen":"2026-09-19"}],"ignored_recurring_series":[{"id":"series","last_seen":"2026-09-20"}]}).to_string())]},
                statement
            ]).await.map_err(failure)?;
            services.executor.execute("SELECT value_json FROM app_settings WHERE id='ignored_events'".into(),vec![]).await.map_err(failure)
        }).unwrap().receive().await.unwrap();
        let ignored: Value = serde_json::from_str(rows[0]["value_json"].as_str().unwrap()).unwrap();
        assert_eq!(
            ignored,
            json!([{"tracking_id":"current","last_seen":"2026-09-20","custom":"preserved"}])
        );
        let mut other = event.clone();
        other.id = "ambiguous".into();
        assert!(migrate_ignored(&[event, other], "now").unwrap().is_none());
        runtime.shutdown().await.unwrap();
    }
}
