use hypr_calendar_interface::EventFilter;
use hypr_google_calendar::{CalendarListEntry as GoogleCalendar, Event as GoogleEvent};
use hypr_outlook_calendar::{Calendar as OutlookCalendar, Event as OutlookEvent};

use crate::error::Error;

pub async fn has_nango_connection(
    api_base_url: &str,
    access_token: &str,
    integration_id: &str,
) -> Result<bool, Error> {
    let client = make_client(api_base_url, access_token)?;

    let response = client
        .list_connections()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    let connections = response.into_inner().connections;
    Ok(connections
        .iter()
        .any(|c| c.integration_id == integration_id))
}

fn make_client(api_base_url: &str, access_token: &str) -> Result<hypr_api_client::Client, Error> {
    let auth_value = format!("Bearer {access_token}").parse()?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;
    Ok(hypr_api_client::Client::new_with_client(api_base_url, http))
}

pub async fn list_google_calendars(
    api_base_url: &str,
    access_token: &str,
) -> Result<Vec<GoogleCalendar>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let response = client
        .google_list_calendars()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().items)
}

pub async fn list_google_events(
    api_base_url: &str,
    access_token: &str,
    filter: EventFilter,
) -> Result<Vec<GoogleEvent>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = hypr_api_client::types::GoogleListEventsRequest {
        calendar_id: filter.calendar_tracking_id,
        time_min: Some(filter.from.to_rfc3339()),
        time_max: Some(filter.to.to_rfc3339()),
        max_results: None,
        page_token: None,
        single_events: Some(true),
        order_by: Some("startTime".to_string()),
    };

    let response = client
        .google_list_events(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().items)
}

pub async fn list_outlook_calendars(
    api_base_url: &str,
    access_token: &str,
) -> Result<Vec<OutlookCalendar>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let response = client
        .outlook_list_calendars()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().value)
}

pub async fn list_outlook_events(
    api_base_url: &str,
    access_token: &str,
    filter: EventFilter,
) -> Result<Vec<OutlookEvent>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = hypr_api_client::types::OutlookListEventsRequest {
        calendar_id: filter.calendar_tracking_id,
        time_min: Some(filter.from.to_rfc3339()),
        time_max: Some(filter.to.to_rfc3339()),
        max_results: None,
        order_by: Some("startTime".to_string()),
    };

    let response = client
        .outlook_list_events(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().value)
}
