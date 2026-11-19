use anlg_api_auth::AuthContext;
use anlg_api_nango::{NangoConnectionError, NangoConnectionState, NangoIntegrationId, Outlook};
use anlg_outlook_calendar::{ListCalendarsResponse, ListEventsResponse, OutlookCalendarClient};
use axum::{Extension, Json};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{CalendarError, Result, map_provider_error};

#[derive(Debug, Deserialize, ToSchema)]
pub struct OutlookListCalendarsRequest {
    pub connection_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct OutlookListEventsRequest {
    pub connection_id: String,
    pub calendar_id: String,
    #[serde(default)]
    pub time_min: Option<String>,
    #[serde(default)]
    pub time_max: Option<String>,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub order_by: Option<String>,
}

#[utoipa::path(
    post,
    path = "/outlook/list-calendars",
    operation_id = "outlook_list_calendars",
    request_body = OutlookListCalendarsRequest,
    responses(
        (status = 200, description = "Outlook calendars fetched", body = ListCalendarsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 424, description = "Calendar connection requires reconnect or mailbox is unavailable"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "calendar",
)]
pub async fn list_calendars(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<OutlookListCalendarsRequest>,
) -> Result<Json<ListCalendarsResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Outlook::ID,
            &req.connection_id,
        )
        .await
        .map_err(connection_error)?
        .with_retries(0)
        .with_base_url_override("https://graph.microsoft.com/v1.0");

    let client = OutlookCalendarClient::new(http);

    let response = match client.list_calendars().await {
        Ok(response) => response,
        Err(e) => {
            return Err(outlook_read_error(&nango_state, &req.connection_id, e).await);
        }
    };

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/outlook/list-events",
    operation_id = "outlook_list_events",
    request_body = OutlookListEventsRequest,
    responses(
        (status = 200, description = "Outlook events fetched", body = ListEventsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 424, description = "Calendar connection requires reconnect or mailbox is unavailable"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "calendar",
)]
pub async fn list_events(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<OutlookListEventsRequest>,
) -> Result<Json<ListEventsResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            Outlook::ID,
            &req.connection_id,
        )
        .await
        .map_err(connection_error)?
        .with_retries(0)
        .with_base_url_override("https://graph.microsoft.com/v1.0");

    let client = OutlookCalendarClient::new(http);

    let start_date_time = req
        .time_min
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| CalendarError::BadRequest(format!("Invalid time_min: {e}")))
        })
        .transpose()?;

    let end_date_time = req
        .time_max
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| CalendarError::BadRequest(format!("Invalid time_max: {e}")))
        })
        .transpose()?;

    let order_by = req.order_by.as_deref().map(|s| match s {
        "startTime" => "start/dateTime".to_string(),
        "updated" => "lastModifiedDateTime".to_string(),
        other => other.to_string(),
    });

    let outlook_req = anlg_outlook_calendar::ListEventsRequest {
        calendar_id: req.calendar_id,
        start_date_time,
        end_date_time,
        top: req.max_results,
        order_by,
        ..Default::default()
    };

    let response = match client.list_events(outlook_req).await {
        Ok(response) => response,
        Err(e) => {
            return Err(outlook_read_error(&nango_state, &req.connection_id, e).await);
        }
    };

    Ok(Json(response))
}

fn connection_error(err: NangoConnectionError) -> CalendarError {
    if matches!(err, NangoConnectionError::Database(_)) {
        tracing::warn!(
            anarlog.error.stage = "connection_lookup",
            error.type = "database",
            "outlook_calendar_read_failed"
        );
    }
    err.into()
}

async fn outlook_read_error(
    state: &NangoConnectionState,
    connection_id: &str,
    err: anlg_outlook_calendar::Error,
) -> CalendarError {
    use anlg_outlook_calendar::Error;
    let mapped = map_provider_error(state, Outlook::ID, connection_id, &err).await;
    if matches!(mapped, CalendarError::Internal(_)) {
        let (stage, kind, status) = match &err {
            Error::InvalidNextLink => ("pagination", "invalid_next_link", None),
            Error::RepeatedNextLink => ("pagination", "repeated_next_link", None),
            Error::TooManyPages => ("pagination", "too_many_pages", None),
            Error::Json(_) => ("decode", "invalid_graph_response", None),
            Error::Http(source) => match source.downcast_ref::<anlg_nango::Error>() {
                Some(anlg_nango::Error::Api(status, _)) => {
                    ("upstream", "provider_http", Some(*status))
                }
                _ => ("transport", "http_client", None),
            },
        };
        tracing::warn!(
            anarlog.error.stage = stage,
            error.type = kind,
            upstream.status_code = status,
            "outlook_calendar_read_failed"
        );
    }
    mapped
}
