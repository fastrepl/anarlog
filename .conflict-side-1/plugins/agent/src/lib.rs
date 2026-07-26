use tauri::Wry;

mod commands;
mod error;
mod ext;

pub use error::{Error, Result};
pub use ext::AgentPluginExt;
pub use hypr_agent_core::*;

const PLUGIN_NAME: &str = "agent";

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::health_check::<Wry>,
            commands::install_cli::<Wry>,
            commands::uninstall_cli::<Wry>,
        ])
        .typ::<hypr_agent_core::ProviderKind>()
        .typ::<hypr_agent_core::ProviderHealthStatus>()
        .typ::<hypr_agent_core::ProviderAuthStatus>()
        .typ::<hypr_agent_core::ProviderHealth>()
        .typ::<hypr_agent_core::HealthCheckResponse>()
        .typ::<hypr_agent_core::InstallCliRequest>()
        .typ::<hypr_agent_core::InstallCliResponse>()
        .typ::<hypr_agent_core::UninstallCliRequest>()
        .typ::<hypr_agent_core::UninstallCliResponse>()
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(|_app, _api| {
            hypr_agent_core::upgrade_hooks();
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
}
