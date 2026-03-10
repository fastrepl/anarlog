use axum::{Extension, Json};
use hypr_api_auth::AuthContext;
use hypr_api_nango::{GoogleCalendar, NangoConnectionState, NangoIntegrationId};
use hypr_google_calendar::{
    EventOrderBy, EventType, GoogleCalendarClient, ListCalendarsResponse, ListEventsResponse,
};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{CalendarError, Result};

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListCalendarsRequest {
    pub connection_id: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListEventsRequest {
    pub connection_id: String,
    pub calendar_id: String,
    #[serde(default)]
    pub time_min: Option<String>,
    #[serde(default)]
    pub time_max: Option<String>,
    #[serde(default)]
    pub max_results: Option<u32>,
    #[serde(default)]
    pub page_token: Option<String>,
    #[serde(default)]
    pub single_events: Option<bool>,
    #[serde(default)]
    pub order_by: Option<String>,
}

#[utoipa::path(
    post,
    path = "/google/list-calendars",
    operation_id = "google_list_calendars",
    request_body = GoogleListCalendarsRequest,
    responses(
        (status = 200, description = "Google calendars fetched", body = ListCalendarsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "calendar",
)]
pub async fn list_calendars(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleListCalendarsRequest>,
) -> Result<Json<ListCalendarsResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleCalendar::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleCalendarClient::new(http);

    let response = client
        .list_calendars()
        .await
        .map_err(|e| CalendarError::Internal(e.to_string()))?;

    Ok(Json(response))
}

#[utoipa::path(
    post,
    path = "/google/list-events",
    operation_id = "google_list_events",
    request_body = GoogleListEventsRequest,
    responses(
        (status = 200, description = "Google events fetched", body = ListEventsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "calendar",
)]
pub async fn list_events(
    Extension(auth): Extension<AuthContext>,
    Extension(nango_state): Extension<NangoConnectionState>,
    Json(req): Json<GoogleListEventsRequest>,
) -> Result<Json<ListEventsResponse>> {
    let http = nango_state
        .build_http_client(
            &auth.token,
            &auth.claims.sub,
            GoogleCalendar::ID,
            &req.connection_id,
        )
        .await?;

    let client = GoogleCalendarClient::new(http);

    let time_min = req
        .time_min
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| CalendarError::BadRequest(format!("Invalid time_min: {e}")))
        })
        .transpose()?;

    let time_max = req
        .time_max
        .as_deref()
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .map_err(|e| CalendarError::BadRequest(format!("Invalid time_max: {e}")))
        })
        .transpose()?;

    let order_by = req
        .order_by
        .as_deref()
        .map(|s| match s {
            "startTime" => Ok(EventOrderBy::StartTime),
            "updated" => Ok(EventOrderBy::Updated),
            other => Err(CalendarError::BadRequest(format!(
                "Invalid order_by: {other}"
            ))),
        })
        .transpose()?;

    let google_req = hypr_google_calendar::ListEventsRequest {
        calendar_id: req.calendar_id,
        time_min,
        time_max,
        max_results: req.max_results,
        page_token: req.page_token,
        single_events: req.single_events,
        order_by,
        event_types: Some(vec![EventType::Default]),
        ..Default::default()
    };

    let response = client
        .list_events(google_req)
        .await
        .map_err(|e| CalendarError::Internal(e.to_string()))?;

    Ok(Json(response))
}
