use std::sync::atomic::{AtomicUsize, Ordering};

use apple_calendar::Handle;
use apple_calendar::types::EventFilter;
use chrono::Utc;

static BUG_COUNT: AtomicUsize = AtomicUsize::new(0);

fn list_calendars_count() -> usize {
    let handle = Handle::new();
    handle.list_calendars().map(|c| c.len()).unwrap_or(0)
}

fn list_events_for(calendar_id: &str) -> Result<usize, String> {
    let handle = Handle::new();
    let filter = EventFilter {
        from: Utc::now() - chrono::Duration::days(1),
        to: Utc::now() + chrono::Duration::days(1),
        calendar_tracking_id: calendar_id.to_string(),
    };
    handle
        .list_events(filter)
        .map(|e| e.len())
        .map_err(|e| e.to_string())
}

fn main() {
    // Set up notification observer like the real app
    apple_calendar::setup_change_notification(|| {});
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Get real calendar IDs so some list_events calls succeed (heavier work)
    let handle = Handle::new();
    let real_ids: Vec<String> = handle
        .list_calendars()
        .unwrap()
        .iter()
        .map(|c| c.id.clone())
        .collect();
    let baseline = real_ids.len();
    println!("Baseline: {baseline} calendars: {real_ids:?}");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(8)
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        // Simulate what the real app does on each sync cycle:
        // 1. list_events for each tracked calendar (concurrently)
        //    - includes a bogus ID (deleted calendar)
        // 2. list_calendars to get latest state
        //
        // Run many rounds to make the race reliable.
        for round in 0..100 {
            let mut futs = vec![];

            // Fire list_events for every real calendar + a bogus one, all at once
            for id in &real_ids {
                let id = id.clone();
                futs.push(tokio::task::spawn_blocking(move || {
                    let _ = list_events_for(&id);
                }));
            }
            futs.push(tokio::task::spawn_blocking(|| {
                let _ = list_events_for("nonexistent-calendar-id");
            }));

            // Concurrently also do list_calendars (the real app does this in the same cycle)
            let cal_fut = tokio::task::spawn_blocking(list_calendars_count);

            // Await everything
            for f in futs {
                let _ = f.await;
            }
            let count = cal_fut.await.unwrap();

            if count == 0 {
                BUG_COUNT.fetch_add(1, Ordering::SeqCst);
                println!("round {round}: BUG! list_calendars returned 0 (expected {baseline})");
            } else if count != baseline {
                println!("round {round}: got {count} calendars (expected {baseline})");
            }
        }

        let bugs = BUG_COUNT.load(Ordering::SeqCst);
        println!("\nResult: {bugs}/100 rounds triggered the bug");
    });
}
