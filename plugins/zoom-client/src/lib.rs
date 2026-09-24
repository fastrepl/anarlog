use tauri::Manager;

mod commands;
mod error;
mod ext;
mod types;

pub use error::{Error, Result};
pub use ext::*;
pub use types::*;

const PLUGIN_NAME: &str = "zoom-client";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::is_available,
            commands::join_meeting::<tauri::Wry>,
            commands::leave_meeting::<tauri::Wry>,
            commands::get_status::<tauri::Wry>,
        ])
        .events(tauri_specta::collect_events![ZoomClientEvent])
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
    fn maps_capture_payloads_to_client_events() {
        use anlg_meeting_capture::{ActiveSpeakers, CaptureEventPayload, Participant};

        let participant = Participant {
            id: "16778240".into(),
            display_name: Some("John".into()),
            email: None,
        };
        assert_eq!(
            ZoomClientEvent::from_capture(
                "s1",
                CaptureEventPayload::ParticipantUpserted(participant.clone())
            ),
            Some(ZoomClientEvent::ParticipantUpserted {
                session_id: "s1".into(),
                participant: participant.into(),
            })
        );
        assert_eq!(
            ZoomClientEvent::from_capture(
                "s1",
                CaptureEventPayload::ActiveSpeakers(ActiveSpeakers {
                    at_ms: 10,
                    participant_ids: vec!["16778240".into()],
                })
            ),
            Some(ZoomClientEvent::ActiveSpeakers {
                session_id: "s1".into(),
                speakers: ZoomActiveSpeakers {
                    at_ms: 10,
                    participant_ids: vec!["16778240".into()],
                },
            })
        );
    }
}
