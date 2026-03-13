use std::time::Duration;

use tauri_plugin_detect::env::test_support::TestEnv;
use tauri_plugin_detect::handler::handle_detect_event;
use tauri_plugin_detect::{DetectEvent, ProcessorState};

fn zoom() -> hypr_detect::InstalledApp {
    hypr_detect::InstalledApp {
        id: "us.zoom.xos".to_string(),
        name: "zoom.us".to_string(),
    }
}

fn aqua_voice() -> hypr_detect::InstalledApp {
    hypr_detect::InstalledApp {
        id: "com.electron.aqua-voice".to_string(),
        name: "Aqua Voice".to_string(),
    }
}

fn slack() -> hypr_detect::InstalledApp {
    hypr_detect::InstalledApp {
        id: "com.tinyspeck.slackmacgap".to_string(),
        name: "Slack".to_string(),
    }
}

struct Harness {
    env: TestEnv,
    state: ProcessorState,
}

impl Harness {
    fn new() -> Self {
        Self {
            env: TestEnv::new(),
            state: ProcessorState::default(),
        }
    }

    fn mic_started(&self, app: hypr_detect::InstalledApp) {
        handle_detect_event(
            &self.env,
            &self.state,
            hypr_detect::DetectEvent::MicStarted(vec![app]),
        );
    }

    fn mic_stopped(&self, app: hypr_detect::InstalledApp) {
        handle_detect_event(
            &self.env,
            &self.state,
            hypr_detect::DetectEvent::MicStopped(vec![app]),
        );
    }

    async fn settle(&self) {
        for _ in 0..100 {
            tokio::task::yield_now().await;
        }
    }

    async fn advance_secs(&self, secs: u64) {
        self.settle().await;
        tokio::time::advance(Duration::from_secs(secs)).await;
        self.settle().await;
    }

    fn take_events(&self) -> Vec<DetectEvent> {
        std::mem::take(&mut self.env.events.lock().unwrap())
    }
}

#[tokio::test(start_paused = true)]
async fn test_mic_detected_after_delay() {
    let h = Harness::new();

    h.mic_started(zoom());
    assert!(h.take_events().is_empty(), "nothing emitted immediately");

    h.advance_secs(15).await;

    let events = h.take_events();
    assert_eq!(events.len(), 1, "expected one MicDetected event");
    assert!(
        matches!(
            &events[0],
            DetectEvent::MicDetected { apps, duration_secs, .. }
                if apps[0].id == "us.zoom.xos" && *duration_secs == 15
        ),
        "expected MicDetected with zoom app and duration_secs=15"
    );
}

#[tokio::test(start_paused = true)]
async fn test_filtered_app_no_event() {
    let h = Harness::new();

    h.mic_started(aqua_voice());
    assert!(
        h.take_events().is_empty(),
        "categorized app should not start a timer"
    );

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "categorized app should not emit MicDetected"
    );
}

#[tokio::test(start_paused = true)]
async fn test_cancel_before_timer() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(3).await;
    h.mic_stopped(zoom());
    h.take_events();

    h.advance_secs(15).await;

    assert!(
        h.take_events().is_empty(),
        "cancelled timer should not emit"
    );
}

#[tokio::test(start_paused = true)]
async fn test_user_ignored_app_no_timer() {
    let h = Harness::new();

    {
        let mut guard = h.state.lock().unwrap();
        guard
            .policy
            .user_ignored_bundle_ids
            .insert("us.zoom.xos".to_string());
    }

    h.mic_started(zoom());
    assert!(
        h.take_events().is_empty(),
        "user-ignored app should not start a timer"
    );

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "user-ignored app should not trigger timer"
    );
}

#[tokio::test(start_paused = true)]
async fn test_full_scenario_zoom_and_dictation() {
    let h = Harness::new();

    h.mic_started(zoom());
    assert!(h.take_events().is_empty(), "nothing emitted immediately");

    h.mic_started(aqua_voice());
    assert!(
        h.take_events().is_empty(),
        "dictation app should be filtered"
    );

    h.mic_stopped(aqua_voice());
    assert!(
        h.take_events().is_empty(),
        "dictation app stop should be filtered"
    );

    h.advance_secs(15).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "zoom MicDetected should fire");
    assert!(matches!(&events[0], DetectEvent::MicDetected { .. }));

    h.mic_stopped(zoom());
    assert_eq!(h.take_events().len(), 1, "zoom should emit MicStopped");
}

#[tokio::test(start_paused = true)]
async fn test_dnd_suppresses_mic_detected() {
    let h = Harness::new();
    h.env.set_dnd(true);
    {
        let mut guard = h.state.lock().unwrap();
        guard.policy.respect_dnd = true;
    }

    h.mic_started(zoom());
    assert!(h.take_events().is_empty(), "nothing emitted immediately");

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "DnD should suppress MicDetected at emit time"
    );
}

#[tokio::test(start_paused = true)]
async fn test_stop_and_restart_creates_new_timer() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(3).await;
    h.mic_stopped(zoom());
    h.take_events();

    h.mic_started(zoom());

    h.advance_secs(10).await;
    assert!(
        h.take_events().is_empty(),
        "new timer should not have fired yet (only 10s since restart)"
    );

    h.advance_secs(5).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "timer should fire 15s after restart");
    assert!(matches!(
        &events[0],
        DetectEvent::MicDetected { apps, .. } if apps[0].id == "us.zoom.xos"
    ));
}

#[tokio::test(start_paused = true)]
async fn test_duplicate_mic_started_no_timer_reset() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(10).await;
    h.mic_started(zoom());

    h.advance_secs(5).await;
    let events = h.take_events();
    assert_eq!(
        events.len(),
        1,
        "timer fires 15s from original start, not from duplicate"
    );
    assert!(matches!(&events[0], DetectEvent::MicDetected { .. }));
}

#[tokio::test(start_paused = true)]
async fn test_multiple_apps_independent_timers() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(3).await;
    h.mic_started(slack());

    h.mic_stopped(zoom());
    h.take_events();

    h.advance_secs(10).await;
    assert!(
        h.take_events().is_empty(),
        "zoom cancelled, slack not yet at 15s"
    );

    h.advance_secs(5).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "only slack timer should fire");
    assert!(matches!(
        &events[0],
        DetectEvent::MicDetected { apps, .. }
            if apps[0].id == "com.tinyspeck.slackmacgap"
    ));
}

#[tokio::test(start_paused = true)]
async fn test_ignore_during_active_tracking_cancels_timer() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(3).await;

    {
        let mut guard = h.state.lock().unwrap();
        guard.mic_usage_tracker.cancel_app("us.zoom.xos");
        guard
            .policy
            .user_ignored_bundle_ids
            .insert("us.zoom.xos".to_string());
    }

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "timer should be cancelled when app is added to ignore list"
    );
}

#[tokio::test(start_paused = true)]
async fn test_cooldown_suppresses_repeated_notifications() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(15).await;
    assert_eq!(h.take_events().len(), 1, "first notification should fire");

    h.mic_stopped(zoom());
    h.take_events();
    h.mic_started(zoom());

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "second notification suppressed by cooldown"
    );
}

#[tokio::test(start_paused = true)]
async fn test_cooldown_expires_after_ten_minutes() {
    let h = Harness::new();

    h.mic_started(zoom());

    h.advance_secs(15).await;
    assert_eq!(h.take_events().len(), 1, "first notification fires");

    h.mic_stopped(zoom());
    h.take_events();

    h.advance_secs(60 * 10).await;

    h.mic_started(zoom());

    h.advance_secs(15).await;
    let events = h.take_events();
    assert_eq!(
        events.len(),
        1,
        "notification fires again after cooldown expires"
    );
}

#[tokio::test(start_paused = true)]
async fn test_detect_disabled_mid_flight_suppresses_mic_detected() {
    let h = Harness::new();

    h.mic_started(zoom());
    assert!(h.take_events().is_empty(), "nothing emitted immediately");

    h.env.set_detect_enabled(false);

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "MicDetected should be suppressed when detect is disabled after timer started"
    );
}

#[tokio::test(start_paused = true)]
async fn test_mic_stopped_with_detect_disabled_cancels_timers() {
    let h = Harness::new();

    h.mic_started(zoom());
    assert!(h.take_events().is_empty(), "nothing emitted immediately");

    h.advance_secs(3).await;
    h.env.set_detect_enabled(false);
    h.mic_stopped(zoom());
    h.take_events();

    h.env.set_detect_enabled(true);

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "timer should have been cancelled by MicStopped even though detect was disabled"
    );
}

#[tokio::test(start_paused = true)]
async fn test_cooldown_is_per_app() {
    let h = Harness::new();

    h.mic_started(zoom());
    h.advance_secs(15).await;
    assert_eq!(h.take_events().len(), 1, "zoom notification fires");

    h.mic_started(slack());
    h.advance_secs(15).await;
    let events = h.take_events();
    assert_eq!(
        events.len(),
        1,
        "slack notification fires despite zoom cooldown"
    );
    assert!(matches!(
        &events[0],
        DetectEvent::MicDetected { apps, .. }
            if apps[0].id == "com.tinyspeck.slackmacgap"
    ));
}

#[tokio::test(start_paused = true)]
async fn test_mic_started_with_detect_disabled_is_noop() {
    let h = Harness::new();
    h.env.set_detect_enabled(false);

    h.mic_started(zoom());
    h.advance_secs(15).await;

    assert!(
        h.take_events().is_empty(),
        "no timer should start when detect is disabled"
    );
}

#[tokio::test(start_paused = true)]
async fn test_dnd_suppresses_mic_stopped_notification() {
    let h = Harness::new();

    h.mic_started(zoom());
    h.advance_secs(15).await;
    assert_eq!(h.take_events().len(), 1, "MicDetected fires");

    {
        let mut guard = h.state.lock().unwrap();
        guard.policy.respect_dnd = true;
    }
    h.env.set_dnd(true);

    h.mic_stopped(zoom());
    assert!(
        h.take_events().is_empty(),
        "MicStopped should be suppressed by DnD"
    );
}

#[tokio::test(start_paused = true)]
async fn test_multiple_apps_start_simultaneously() {
    let h = Harness::new();

    handle_detect_event(
        &h.env,
        &h.state,
        hypr_detect::DetectEvent::MicStarted(vec![zoom(), slack()]),
    );

    h.advance_secs(15).await;

    let events = h.take_events();
    assert_eq!(events.len(), 2, "both apps should fire independently");

    let mut app_ids: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            DetectEvent::MicDetected { apps, .. } => Some(apps[0].id.clone()),
            _ => None,
        })
        .collect();
    app_ids.sort();
    assert_eq!(app_ids, vec!["com.tinyspeck.slackmacgap", "us.zoom.xos"]);
}

#[tokio::test(start_paused = true)]
async fn test_threshold_change_affects_new_timers() {
    let h = Harness::new();

    {
        let mut guard = h.state.lock().unwrap();
        guard.mic_active_threshold_secs = 30;
    }

    h.mic_started(zoom());

    h.advance_secs(15).await;
    assert!(
        h.take_events().is_empty(),
        "should not fire at 15s with 30s threshold"
    );

    h.advance_secs(15).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "should fire at 30s");
    assert!(matches!(
        &events[0],
        DetectEvent::MicDetected { duration_secs, .. } if *duration_secs == 30
    ));
}

#[tokio::test(start_paused = true)]
async fn test_detect_re_enabled_after_disabled_starts_fresh() {
    let h = Harness::new();
    h.env.set_detect_enabled(false);

    h.mic_started(zoom());
    h.advance_secs(15).await;
    assert!(h.take_events().is_empty(), "disabled, no event");

    h.env.set_detect_enabled(true);
    h.mic_started(zoom());

    h.advance_secs(15).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "re-enabled should start fresh timer");
}

#[tokio::test(start_paused = true)]
async fn test_stop_all_apps_simultaneously() {
    let h = Harness::new();

    h.mic_started(zoom());
    h.mic_started(slack());

    h.advance_secs(3).await;

    handle_detect_event(
        &h.env,
        &h.state,
        hypr_detect::DetectEvent::MicStopped(vec![zoom(), slack()]),
    );
    h.take_events();

    h.advance_secs(15).await;
    assert!(h.take_events().is_empty(), "all timers should be cancelled");
}

#[tokio::test(start_paused = true)]
async fn test_rapid_start_stop_start_within_threshold() {
    let h = Harness::new();

    h.mic_started(zoom());
    h.advance_secs(5).await;
    h.mic_stopped(zoom());
    h.take_events();

    h.advance_secs(2).await;
    h.mic_started(zoom());

    h.advance_secs(15).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "new timer should fire after restart");
}

#[tokio::test(start_paused = true)]
async fn test_user_ignore_added_mid_flight_for_one_of_two_apps() {
    let h = Harness::new();

    h.mic_started(zoom());
    h.mic_started(slack());

    h.advance_secs(5).await;

    {
        let mut guard = h.state.lock().unwrap();
        guard.mic_usage_tracker.cancel_app("us.zoom.xos");
        guard
            .policy
            .user_ignored_bundle_ids
            .insert("us.zoom.xos".to_string());
    }

    h.advance_secs(10).await;
    let events = h.take_events();
    assert_eq!(events.len(), 1, "only slack should fire");
    assert!(matches!(
        &events[0],
        DetectEvent::MicDetected { apps, .. }
            if apps[0].id == "com.tinyspeck.slackmacgap"
    ));
}

#[tokio::test(start_paused = true)]
async fn test_dnd_toggled_mid_flight_after_timer_started() {
    let h = Harness::new();

    {
        let mut guard = h.state.lock().unwrap();
        guard.policy.respect_dnd = true;
    }

    h.mic_started(zoom());

    h.advance_secs(5).await;
    h.env.set_dnd(true);

    h.advance_secs(10).await;
    assert!(
        h.take_events().is_empty(),
        "DnD enabled mid-flight should suppress MicDetected"
    );
}

#[tokio::test(start_paused = true)]
async fn test_dnd_toggled_off_before_timer_fires() {
    let h = Harness::new();

    {
        let mut guard = h.state.lock().unwrap();
        guard.policy.respect_dnd = true;
    }
    h.env.set_dnd(true);

    h.mic_started(zoom());

    h.advance_secs(5).await;
    h.env.set_dnd(false);

    h.advance_secs(10).await;
    let events = h.take_events();
    assert_eq!(
        events.len(),
        1,
        "DnD turned off before timer fires should allow MicDetected"
    );
}
