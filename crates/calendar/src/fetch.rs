use std::time::Duration;

use anlg_calendar_interface::EventFilter;
use anlg_google_calendar::{CalendarListEntry as GoogleCalendar, Event as GoogleEvent};
use anlg_outlook_calendar::{Calendar as OutlookCalendar, Event as OutlookEvent};

use crate::error::Error;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub async fn list_all_connection_ids(
    api_base_url: &str,
    access_token: &str,
) -> Result<Vec<(String, Vec<String>)>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let response = client
        .list_connections()
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    let connections = response.into_inner().connections;
    let mut map = std::collections::HashMap::<String, Vec<String>>::new();
    for c in &connections {
        map.entry(c.integration_id.clone())
            .or_default()
            .push(c.connection_id.clone());
    }

    Ok(map.into_iter().collect())
}

fn make_client(api_base_url: &str, access_token: &str) -> Result<anlg_api_client::Client, Error> {
    let auth_value = format!("Bearer {access_token}").parse()?;
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::AUTHORIZATION, auth_value);
    let http = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()?;
    Ok(anlg_api_client::Client::new_with_client(api_base_url, http))
}

pub async fn list_google_calendars(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
) -> Result<Vec<GoogleCalendar>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = anlg_api_client::types::GoogleListCalendarsRequest {
        connection_id: connection_id.to_string(),
    };

    let response = client
        .google_list_calendars(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().items)
}

pub async fn list_google_events(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
    filter: EventFilter,
) -> Result<Vec<GoogleEvent>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let mut body = anlg_api_client::types::GoogleListEventsRequest {
        connection_id: connection_id.to_string(),
        calendar_id: filter.calendar_tracking_id,
        time_min: Some(filter.from.to_rfc3339()),
        time_max: Some(filter.to.to_rfc3339()),
        max_results: None,
        page_token: None,
        single_events: Some(true),
        order_by: Some("startTime".to_string()),
    };

    let mut events = Vec::new();
    loop {
        let response = client
            .google_list_events(&body)
            .await
            .map_err(|e| Error::Api(e.to_string()))?
            .into_inner();
        events.extend(response.items);
        body.page_token = response.next_page_token;
        // Google can return an empty page even when more events match the query.
        if body.page_token.is_none() {
            return Ok(events);
        }
    }
}

pub async fn list_outlook_calendars(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
) -> Result<Vec<OutlookCalendar>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = anlg_api_client::types::OutlookListCalendarsRequest {
        connection_id: connection_id.to_string(),
    };

    let response = client
        .outlook_list_calendars(&body)
        .await
        .map_err(|e| Error::Api(e.to_string()))?;

    Ok(response.into_inner().value)
}

pub async fn list_outlook_events(
    api_base_url: &str,
    access_token: &str,
    connection_id: &str,
    filter: EventFilter,
) -> Result<Vec<OutlookEvent>, Error> {
    let client = make_client(api_base_url, access_token)?;

    let body = anlg_api_client::types::OutlookListEventsRequest {
        connection_id: connection_id.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{body_partial_json, method, path},
    };

    #[tokio::test]
    async fn google_events_retains_free_busy_instances_after_empty_page() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(|request: &wiremock::Request| {
                request
                    .body_json::<serde_json::Value>()
                    .unwrap()
                    .get("page_token")
                    .is_none_or(serde_json::Value::is_null)
            })
            .and(path("/calendar/google/list-events"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [],
                "nextPageToken": "next-page"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/calendar/google/list-events"))
            .and(body_partial_json(
                serde_json::json!({"page_token": "next-page"}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": [{
                    "kind": "calendar#event",
                    "etag": "\"busy\"",
                    "id": "busyseries_20260921T200000Z",
                    "status": "confirmed",
                    "updated": "2026-09-01T00:00:00Z",
                    "start": {"dateTime": "2026-09-21T16:00:00-04:00"},
                    "end": {"dateTime": "2026-09-21T17:00:00-04:00"}
                }],
                "nextPageToken": "last-page"
            })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/calendar/google/list-events"))
            .and(body_partial_json(
                serde_json::json!({"page_token": "last-page"}),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "items": []
            })))
            .expect(1)
            .mount(&server)
            .await;

        let events = list_google_events(
            &server.uri(),
            "test-token",
            "test-connection",
            EventFilter {
                calendar_tracking_id: "shared-calendar".into(),
                from: "2026-08-31T04:00:00Z".parse().unwrap(),
                to: "2026-10-05T04:00:00Z".parse().unwrap(),
            },
        )
        .await
        .unwrap();
        let events = crate::convert::convert_google_events(events, "shared-calendar");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, "busyseries_20260921T200000Z");
        assert_eq!(events[0].started_at, "2026-09-21T16:00:00-04:00");
        assert!(events[0].title.is_empty());
        assert!(events[0].attendees.is_empty());
    }

    #[tokio::test]
    async fn google_events_rejects_partial_results_when_next_page_fails() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/calendar/google/list-events"))
            .respond_with(|request: &wiremock::Request| {
                let body = request.body_json::<serde_json::Value>().unwrap();
                if body.get("page_token").is_some_and(|value| !value.is_null()) {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "items": [{"id": "busy-event"}],
                        "nextPageToken": "next-page"
                    }))
                }
            })
            .mount(&server)
            .await;

        let result = list_google_events(
            &server.uri(),
            "test-token",
            "test-connection",
            EventFilter {
                calendar_tracking_id: "shared-calendar".into(),
                from: "2026-08-31T04:00:00Z".parse().unwrap(),
                to: "2026-10-05T04:00:00Z".parse().unwrap(),
            },
        )
        .await;

        assert!(matches!(result, Err(Error::Api(_))));
    }
}
