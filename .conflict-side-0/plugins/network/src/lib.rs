use ractor::{Actor, ActorCell};
use tauri::Manager;

mod actor;
pub mod event;

pub use actor::*;
pub use event::*;

const PLUGIN_NAME: &str = "network";

#[derive(Default)]
pub struct InitOptions {
    pub parent_supervisor: Option<ActorCell>,
}

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .events(tauri_specta::collect_events![NetworkStatusEvent])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

pub fn init(options: InitOptions) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _api| {
            specta_builder.mount_events(app);

            let app_handle = app.app_handle().clone();
            let parent = options.parent_supervisor.clone();

            tauri::async_runtime::spawn(async move {
                match Actor::spawn(
                    Some(NetworkActor::name()),
                    NetworkActor,
                    NetworkArgs { app: app_handle },
                )
                .await
                {
                    Ok((actor_ref, _)) => {
                        if let Some(parent_cell) = parent {
                            actor_ref.get_cell().link(parent_cell);
                        }
                        tracing::info!("network_actor_spawned");
                    }
                    Err(e) => {
                        tracing::error!(?e, "failed_to_spawn_network_actor");
                    }
                }
            });

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
