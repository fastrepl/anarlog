use tauri::Manager;

mod commands;
mod error;
mod ext;
mod types;

pub use error::{Error, Result};
pub use ext::*;
pub use types::*;

const PLUGIN_NAME: &str = "meet-client";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::is_available,
            commands::join_meeting::<tauri::Wry>,
            commands::leave_meeting::<tauri::Wry>,
            commands::get_status::<tauri::Wry>,
            commands::set_selectors::<tauri::Wry>,
            commands::report_observation::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![MeetClientEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);
            assert!(app.manage(ManagedState::default()));
            Ok(())
        })
        .build()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn export_types() {
        const OUTPUT_FILE: &str = "./js/bindings.gen.ts";

        make_specta_builder::<tauri::Wry>()
            .export(
                specta_typescript::Typescript::default()
                    .formatter(specta_typescript::formatter::prettier)
                    .bigint(specta_typescript::BigIntExportBehavior::Number),
                OUTPUT_FILE,
            )
            .unwrap();

        let content = std::fs::read_to_string(OUTPUT_FILE).unwrap();
        std::fs::write(OUTPUT_FILE, format!("// @ts-nocheck\n{content}")).unwrap();
    }

    #[test]
    fn accepts_only_meet_urls() {
        assert!(is_meet_url("https://meet.google.com/abc-defg-hij"));
        assert!(is_meet_url(
            "https://meet.google.com/abc-defg-hij?authuser=0"
        ));
        assert!(!is_meet_url("http://meet.google.com/abc-defg-hij"));
        assert!(!is_meet_url(
            "https://meet.google.com.evil.example/abc-defg-hij"
        ));
        assert!(!is_meet_url("https://zoom.us/j/1"));
    }

    fn app() -> tauri::App<tauri::test::MockRuntime> {
        tauri::test::mock_builder()
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    #[test]
    fn observations_before_join_are_ignored() {
        let app = app();
        app.meet_client()
            .observe(MeetObservation {
                in_meeting: true,
                ended: false,
                participants: vec![],
            })
            .unwrap();
        assert_eq!(app.meet_client().status().state, MeetClientState::Idle);
    }

    #[test]
    fn observations_drive_lifecycle_and_participants() {
        let app = app();
        let client = app.meet_client();
        client.begin_for_test("s1");

        client
            .observe(MeetObservation {
                in_meeting: false,
                ended: false,
                participants: vec![],
            })
            .unwrap();
        assert_eq!(client.status().state, MeetClientState::WaitingForAdmission);

        let john = ObservedParticipant {
            id: "p1".into(),
            display_name: Some("John".into()),
            speaking: true,
        };
        let mina = ObservedParticipant {
            id: "p2".into(),
            display_name: Some("Mina".into()),
            speaking: false,
        };
        client
            .observe(MeetObservation {
                in_meeting: true,
                ended: false,
                participants: vec![john.clone(), mina],
            })
            .unwrap();
        let status = client.status();
        assert_eq!(status.state, MeetClientState::Capturing);
        assert_eq!(status.participants.len(), 2);

        client
            .observe(MeetObservation {
                in_meeting: true,
                ended: false,
                participants: vec![john],
            })
            .unwrap();
        assert_eq!(
            client
                .status()
                .participants
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            vec!["p1"]
        );

        client
            .observe(MeetObservation {
                in_meeting: true,
                ended: true,
                participants: vec![],
            })
            .unwrap();
        assert_eq!(client.status().state, MeetClientState::Ended);
    }

    #[test]
    fn selectors_default_and_override() {
        let app = app();
        assert_eq!(app.meet_client().selectors(), MeetSelectors::default());
        let custom = MeetSelectors {
            tile: ".tile".into(),
            ..MeetSelectors::default()
        };
        app.meet_client().set_selectors(custom.clone()).unwrap();
        assert_eq!(app.meet_client().selectors(), custom);
    }
}
