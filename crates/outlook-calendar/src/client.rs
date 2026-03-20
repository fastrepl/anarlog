use hypr_http::HttpClient;

use crate::error::Error;
use crate::types::{
    CreateEventRequest, Event, ListCalendarsResponse, ListEventsRequest, ListEventsResponse,
};

pub struct OutlookCalendarClient<C> {
    http: C,
}

impl<C: HttpClient> OutlookCalendarClient<C> {
    pub fn new(http: C) -> Self {
        Self { http }
    }

    pub async fn list_calendars(&self) -> Result<ListCalendarsResponse, Error> {
        let bytes = self.http.get("/me/calendars").await.map_err(Error::Http)?;
        let response: ListCalendarsResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }

    pub async fn list_events(&self, req: ListEventsRequest) -> Result<ListEventsResponse, Error> {
        let calendar_id = &req.calendar_id;

        let use_calendar_view = req.start_date_time.is_some() && req.end_date_time.is_some();

        let base_path = if use_calendar_view {
            format!("/me/calendars/{calendar_id}/calendarView")
        } else {
            format!("/me/calendars/{calendar_id}/events")
        };

        let mut query_parts: Vec<String> = Vec::new();

        if let Some(ref start) = req.start_date_time {
            query_parts.push(format!(
                "startDateTime={}",
                urlencoding::encode(&start.to_rfc3339())
            ));
        }
        if let Some(ref end) = req.end_date_time {
            query_parts.push(format!(
                "endDateTime={}",
                urlencoding::encode(&end.to_rfc3339())
            ));
        }
        if let Some(top) = req.top {
            query_parts.push(format!("$top={top}"));
        }
        if let Some(skip) = req.skip {
            query_parts.push(format!("$skip={skip}"));
        }
        if let Some(ref filter) = req.filter {
            query_parts.push(format!("$filter={}", urlencoding::encode(filter)));
        }
        if let Some(ref select) = req.select {
            query_parts.push(format!("$select={}", select.join(",")));
        }
        if let Some(ref order_by) = req.order_by {
            query_parts.push(format!("$orderby={}", urlencoding::encode(order_by)));
        }

        let full_path = if query_parts.is_empty() {
            base_path
        } else {
            format!("{}?{}", base_path, query_parts.join("&"))
        };

        let bytes = self.http.get(&full_path).await.map_err(Error::Http)?;
        let response: ListEventsResponse = serde_json::from_slice(&bytes)?;
        Ok(response)
    }

    pub async fn create_event(&self, req: CreateEventRequest) -> Result<Event, Error> {
        let calendar_id = &req.calendar_id;
        let path = format!("/me/calendars/{calendar_id}/events");

        let body = serde_json::to_vec(&req.event)?;
        let bytes = self
            .http
            .post(&path, body, "application/json")
            .await
            .map_err(Error::Http)?;
        let event: Event = serde_json::from_slice(&bytes)?;
        Ok(event)
    }
}
