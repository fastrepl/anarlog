use axum::Json;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use hypr_api_nango::{GoogleCalendar, NangoConnection};
use hypr_google_calendar::{
    EventOrderBy, GoogleCalendarClient, ListCalendarsResponse, ListEventsResponse,
};
use hypr_nango::OwnedNangoHttpClient;
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{CalendarError, Result};

pub(crate) struct GoogleClient(GoogleCalendarClient<OwnedNangoHttpClient>);

impl<S: Send + Sync> FromRequestParts<S> for GoogleClient {
    type Rejection = CalendarError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        let conn = NangoConnection::<GoogleCalendar>::from_request_parts(parts, state).await?;
        Ok(GoogleClient(GoogleCalendarClient::new(conn.into_http())))
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct GoogleListEventsRequest {
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
    responses(
        (status = 200, description = "Google calendars fetched", body = ListCalendarsResponse),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error"),
    ),
    tag = "calendar",
)]
pub async fn list_calendars(client: GoogleClient) -> Result<Json<ListCalendarsResponse>> {
    let response = client
        .0
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
    client: GoogleClient,
    Json(req): Json<GoogleListEventsRequest>,
) -> Result<Json<ListEventsResponse>> {
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
        ..Default::default()
    };

    let response = client
        .0
        .list_events(google_req)
        .await
        .map_err(|e| CalendarError::Internal(e.to_string()))?;

    Ok(Json(response))
}
