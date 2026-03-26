use std::collections::HashSet;

use hypr_calendar_interface::CalendarListItem;

use super::storage::StoredCalendar;

#[derive(Debug, Clone)]
pub struct CalendarToUpsert {
    pub tracking_id: String,
    pub connection_id: String,
    pub name: String,
    pub color: String,
    pub source: String,
    pub provider: String,
}

#[derive(Debug, Default)]
pub struct CalendarSyncPlan {
    pub to_upsert: Vec<CalendarToUpsert>,
    pub to_delete: Vec<String>,
}

pub fn compute_calendar_sync(
    incoming: &[(String, Vec<CalendarListItem>)],
    existing: &[StoredCalendar],
    provider: &str,
) -> CalendarSyncPlan {
    let mut plan = CalendarSyncPlan::default();

    let incoming_tracking_ids: HashSet<&str> = incoming
        .iter()
        .flat_map(|(_, cals)| cals.iter().map(|c| c.id.as_str()))
        .collect();

    for existing_cal in existing {
        if existing_cal.provider == provider
            && !incoming_tracking_ids.contains(existing_cal.tracking_id.as_str())
        {
            plan.to_delete.push(existing_cal.id.clone());
        }
    }

    for (connection_id, calendars) in incoming {
        for cal in calendars {
            plan.to_upsert.push(CalendarToUpsert {
                tracking_id: cal.id.clone(),
                connection_id: connection_id.clone(),
                name: cal.title.clone(),
                color: cal.color.clone().unwrap_or_else(|| "#888".to_string()),
                source: cal.source.clone().unwrap_or_default(),
                provider: provider.to_string(),
            });
        }
    }

    plan
}
