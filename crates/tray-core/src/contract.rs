use serde::Deserialize;

use crate::TrayScheduleEvent;

#[derive(Debug, Deserialize)]
pub struct Case {
    pub name: String,
    pub events: Vec<TrayScheduleEvent>,
    pub now_ms: f64,
    pub show_events: bool,
    pub is_recording: bool,
    pub recording_title: Option<String>,
    pub expect: Expectation,
}

#[derive(Debug, Deserialize)]
pub struct Expectation {
    pub agenda: Vec<AgendaSection>,
    pub title: Option<String>,
    pub refresh_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct AgendaSection {
    pub label: String,
    pub events: Vec<AgendaEvent>,
}

#[derive(Debug, Deserialize)]
pub struct AgendaEvent {
    pub id: String,
    pub label: String,
}

pub fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("../tests/fixtures/tray_contract.json"))
        .expect("valid tray contract fixture")
}
