use axum::{Extension, Json};
use hypr_api_auth::AuthContext;
use hypr_api_nango::{NangoConnectionState, NangoIntegrationId, Outlook};
use hypr_outlook_calendar::{ListCalendarsResponse, ListEventsResponse, OutlookCalendarClient};
use serde::Deserialize;
use utoipa::ToSchema;

use crate::error::{CalendarError, Result};

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
        .await?
        .with_base_url_override("https://graph.microsoft.com/v1.0");

    let client = OutlookCalendarClient::new(http);

    let response = client
        .list_calendars()
        .await
        .map_err(|e| CalendarError::Internal(e.to_string()))?;

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
        .await?
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

    let outlook_req = hypr_outlook_calendar::ListEventsRequest {
        calendar_id: req.calendar_id,
        start_date_time,
        end_date_time,
        top: req.max_results,
        order_by,
        ..Default::default()
    };

    let response = client
        .list_events(outlook_req)
        .await
        .map_err(|e| CalendarError::Internal(e.to_string()))?;

    Ok(Json(response))
}
