use super::{Event, ListEventFilter, ListEventFilterCommon, ListEventFilterSpecific, UserDatabase};

impl UserDatabase {
    pub async fn get_event(&self, id: impl Into<String>) -> Result<Option<Event>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query("SELECT * FROM events WHERE id = ?", vec![id.into()])
            .await?;

        match rows.next().await? {
            None => Ok(None),
            Some(row) => {
                let event: Event = libsql::de::from_row(&row)?;
                Ok(Some(event))
            }
        }
    }

    pub async fn delete_event(&self, id: impl Into<String>) -> Result<(), crate::Error> {
        let conn = self.conn()?;

        conn.query("DELETE FROM events WHERE id = ?", vec![id.into()])
            .await?;

        Ok(())
    }

    pub async fn update_event(&self, event: Event) -> Result<Event, crate::Error> {
        let conn = self.conn()?;
        let event_id = event.id.clone();

        let mut rows = conn
            .query(
                "UPDATE events SET
                    tracking_id = :tracking_id,
                    calendar_id = :calendar_id,
                    name = :name,
                    note = :note,
                    start_date = :start_date,
                    end_date = :end_date,
                    google_event_url = :google_event_url,
                    participants = :participants,
                    is_recurring = :is_recurring
                WHERE id = :id
                RETURNING *",
                libsql::named_params! {
                    ":id": event.id,
                    ":tracking_id": event.tracking_id,
                    ":calendar_id": event.calendar_id,
                    ":name": event.name,
                    ":note": event.note,
                    ":start_date": event.start_date.to_rfc3339(),
                    ":end_date": event.end_date.to_rfc3339(),
                    ":google_event_url": event.google_event_url,
                    ":participants": event.participants,
                    ":is_recurring": event.is_recurring,
                },
            )
            .await?;

        match rows.next().await? {
            Some(row) => {
                let event: Event = libsql::de::from_row(&row)?;
                Ok(event)
            }
            None => Err(crate::Error::InvalidInput(format!(
                "Event with id '{}' not found",
                event_id
            ))),
        }
    }

    pub async fn upsert_event(&self, event: Event) -> Result<Event, crate::Error> {
        let conn = self.conn()?;

        let mut rows = conn
            .query(
                "INSERT INTO events (
                    id,
                    user_id,
                    tracking_id,
                    calendar_id,
                    name,
                    note,
                    start_date,
                    end_date,
                    google_event_url,
                    participants,
                    is_recurring
                ) VALUES (
                    :id,
                    :user_id,
                    :tracking_id,
                    :calendar_id,
                    :name,
                    :note,
                    :start_date,
                    :end_date,
                    :google_event_url,
                    :participants,
                    :is_recurring
                ) ON CONFLICT(tracking_id) DO UPDATE SET
                    name = :name,
                    note = :note,
                    start_date = :start_date,
                    end_date = :end_date,
                    google_event_url = :google_event_url,
                    participants = :participants,
                    is_recurring = :is_recurring
                RETURNING *",
                libsql::named_params! {
                    ":id": event.id,
                    ":user_id": event.user_id,
                    ":tracking_id": event.tracking_id,
                    ":calendar_id": event.calendar_id,
                    ":name": event.name,
                    ":note": event.note,
                    ":start_date": event.start_date.to_rfc3339(),
                    ":end_date": event.end_date.to_rfc3339(),
                    ":google_event_url": event.google_event_url,
                    ":participants": event.participants,
                    ":is_recurring": event.is_recurring,
                },
            )
            .await?;

        let row = rows.next().await?.unwrap();
        let event: Event = libsql::de::from_row(&row)?;
        Ok(event)
    }

    pub async fn list_events(
        &self,
        filter: Option<ListEventFilter>,
    ) -> Result<Vec<Event>, crate::Error> {
        let conn = self.conn()?;

        let mut rows = match filter {
            Some(ListEventFilter {
                common: ListEventFilterCommon { user_id, limit },
                specific: ListEventFilterSpecific::Simple {},
            }) => {
                conn.query(
                    "SELECT * FROM events WHERE user_id = ? ORDER BY start_date DESC LIMIT ?",
                    vec![user_id, limit.unwrap_or(100).to_string()],
                )
                .await?
            }
            Some(ListEventFilter {
                common: ListEventFilterCommon { user_id, limit },
                specific: ListEventFilterSpecific::Search { query },
            }) => {
                conn.query(
                    "SELECT * FROM events WHERE user_id = ? AND name LIKE ? ORDER BY start_date DESC LIMIT ?",
                    vec![user_id, format!("%{}%", query), limit.unwrap_or(100).to_string()],
                )
                .await?
            }
            Some(ListEventFilter {
                common: ListEventFilterCommon { user_id, limit },
                specific: ListEventFilterSpecific::DateRange { start, end },
            }) => {
                conn.query(
                    "SELECT * FROM events WHERE user_id = ? AND start_date BETWEEN ? AND ? ORDER BY start_date ASC LIMIT ?",
                    vec![user_id, start.to_rfc3339(), end.to_rfc3339(), limit.unwrap_or(100).to_string()],
                )
                .await?
            }
            Some(ListEventFilter {
                common: ListEventFilterCommon { user_id, limit },
                specific: ListEventFilterSpecific::NotAssignedPast {},
            }) => {
                conn.query(
                    "SELECT * FROM events WHERE user_id = ? AND calendar_id IS NULL ORDER BY start_date ASC LIMIT ?",
                    vec![user_id, limit.unwrap_or(100).to_string()],
                )
                .await?
            }
            None => {
                conn.query(
                    "SELECT * FROM events ORDER BY start_date DESC LIMIT 100",
                    (),
                )
                .await?
            }
        };

        let mut items = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            let item: Event = libsql::de::from_row(&row)?;
            items.push(item);
        }
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Calendar, Human, Platform, tests::setup_db};

    #[tokio::test]
    async fn test_events() {
        let db = setup_db().await;

        let human = db
            .upsert_human(Human {
                full_name: Some("yujonglee".to_string()),
                ..Human::default()
            })
            .await
            .unwrap();

        let events = db.list_events(None).await.unwrap();
        assert_eq!(events.len(), 0);

        let calendar = Calendar {
            id: uuid::Uuid::new_v4().to_string(),
            tracking_id: "calendar_test".to_string(),
            user_id: human.id,
            name: "test".to_string(),
            platform: Platform::Google,
            selected: false,
            source: None,
        };

        let calendar = db.upsert_calendar(calendar.clone()).await.unwrap();
        assert_eq!(calendar.tracking_id, "calendar_test");

        let human = db
            .upsert_human(Human {
                full_name: Some("yujonglee".to_string()),
                ..Human::default()
            })
            .await
            .unwrap();

        let event = Event {
            id: uuid::Uuid::new_v4().to_string(),
            user_id: human.id.clone(),
            tracking_id: "event_test".to_string(),
            calendar_id: Some(calendar.id.clone()),
            name: "test".to_string(),
            note: "test".to_string(),
            start_date: chrono::Utc::now(),
            end_date: chrono::Utc::now(),
            google_event_url: None,
            participants: None,
            is_recurring: false,
        };

        let event = db.upsert_event(event).await.unwrap();
        assert_eq!(event.tracking_id, "event_test");
        assert_eq!(event.google_event_url, None);

        let events = db.list_events(None).await.unwrap();
        assert_eq!(events.len(), 1);
    }
}
