use chrono::{DateTime, Utc};

use crate::user_common_derives;

user_common_derives! {
    pub struct Event {
        pub id: String,
        pub user_id: String,
        pub tracking_id: String,
        pub calendar_id: Option<String>,
        pub name: String,
        pub note: String,
        pub start_date: DateTime<Utc>,
        pub end_date: DateTime<Utc>,
        pub google_event_url: Option<String>,
        pub participants: Option<String>,
        pub is_recurring: bool,
    }
}

user_common_derives! {
    pub struct EventParticipant {
        pub name: Option<String>,
        pub email: Option<String>,
    }
}

user_common_derives! {
    pub struct ListEventFilter {
        #[serde(flatten)]
        pub common: ListEventFilterCommon,
        #[serde(flatten)]
        pub specific: ListEventFilterSpecific,
    }
}

user_common_derives! {
    pub struct ListEventFilterCommon {
        pub user_id: String,
        pub limit: Option<u32>,
    }
}

user_common_derives! {
    #[serde(tag = "type")]
    pub enum ListEventFilterSpecific {
        #[serde(rename = "simple")]
        Simple {},
        #[serde(rename = "search")]
        Search { query: String },
        #[serde(rename = "dateRange")]
        DateRange { start: DateTime<Utc>, end: DateTime<Utc> },
        #[serde(rename = "not-assigned-past")]
        NotAssignedPast {},
    }
}
