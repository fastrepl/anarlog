mod commands;
mod error;
mod ext;
mod models;

pub use error::*;
pub use ext::*;
pub use models::*;

const PLUGIN_NAME: &str = "permissions";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::open_permission::<tauri::Wry>,
            commands::check_permission::<tauri::Wry>,
            commands::request_permission::<tauri::Wry>,
            commands::reset_permission::<tauri::Wry>,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |_app, _api| Ok(()))
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

    fn create_app<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::App<R> {
        builder
            .plugin(tauri_plugin_shell::init())
            .plugin(init())
            .build(tauri::test::mock_context(tauri::test::noop_assets()))
            .unwrap()
    }

    macro_rules! permission_request_test {
        ($name:ident, $variant:ident) => {
            #[tokio::test]
            async fn $name() {
                let app = create_app(tauri::test::mock_builder());
                let _ = app.permissions().request(Permission::$variant).await;
            }
        };
    }

    macro_rules! permission_reset_test {
        ($name:ident, $variant:ident) => {
            #[tokio::test]
            async fn $name() {
                let app = create_app(tauri::test::mock_builder());
                let _ = app.permissions().reset(Permission::$variant).await;
            }
        };
    }

    // cargo test --package tauri-plugin-permissions --lib -- test::test_request_calendar --exact --nocapture
    permission_request_test!(test_request_calendar, Calendar);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_request_contacts --exact --nocapture
    permission_request_test!(test_request_contacts, Contacts);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_request_microphone --exact --nocapture
    permission_request_test!(test_request_microphone, Microphone);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_request_system_audio --exact --nocapture
    permission_request_test!(test_request_system_audio, SystemAudio);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_request_accessibility --exact --nocapture
    permission_request_test!(test_request_accessibility, Accessibility);

    // cargo test --package tauri-plugin-permissions --lib -- test::test_reset_calendar --exact --nocapture
    permission_reset_test!(test_reset_calendar, Calendar);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_reset_contacts --exact --nocapture
    permission_reset_test!(test_reset_contacts, Contacts);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_reset_microphone --exact --nocapture
    permission_reset_test!(test_reset_microphone, Microphone);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_reset_system_audio --exact --nocapture
    permission_reset_test!(test_reset_system_audio, SystemAudio);
    // cargo test --package tauri-plugin-permissions --lib -- test::test_reset_accessibility --exact --nocapture
    permission_reset_test!(test_reset_accessibility, Accessibility);
}
