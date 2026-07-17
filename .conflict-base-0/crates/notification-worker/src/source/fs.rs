use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;

use super::{EventSource, UpcomingEvent};

pub struct FsEventSource {
    vault_base: PathBuf,
}

impl FsEventSource {
    pub fn new(vault_base: PathBuf) -> Self {
        Self { vault_base }
    }
}

#[derive(Debug, Deserialize)]
struct StoredParticipant {
    name: Option<String>,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StoredEvent {
    title: String,
    started_at: String,
    ended_at: Option<String>,
    #[serde(default)]
    participants: Vec<StoredParticipant>,
}

impl EventSource for FsEventSource {
    fn upcoming_events(
        &self,
        within: Duration,
    ) -> Pin<
        Box<dyn std::future::Future<Output = Result<Vec<UpcomingEvent>, crate::Error>> + Send + '_>,
    > {
        let path = self.vault_base.join("events.json");

        Box::pin(async move {
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
                Err(e) => return Err(e.into()),
            };

            let stored: HashMap<String, StoredEvent> = serde_json::from_str(&content)?;

            let now = Utc::now();
            let cutoff = now + within;

            let events = stored
                .into_iter()
                .filter_map(|(event_id, event)| {
                    let started_at = DateTime::parse_from_rfc3339(&event.started_at)
                        .ok()?
                        .with_timezone(&Utc);

                    if started_at <= now || started_at > cutoff {
                        return None;
                    }

                    let ended_at = event
                        .ended_at
                        .as_deref()
                        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                        .map(|dt| dt.with_timezone(&Utc));

                    let participants = event
                        .participants
                        .into_iter()
                        .filter_map(|p| p.name.or(p.email))
                        .collect();

                    Some(UpcomingEvent {
                        event_id,
                        title: event.title,
                        started_at,
                        ended_at,
                        participants,
                    })
                })
                .collect();

            Ok(events)
        })
    }
}
