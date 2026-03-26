mod commands;
mod relay;
mod server;

use std::net::SocketAddr;

use tauri::{AppHandle, Manager, Runtime};

const PLUGIN_NAME: &str = "relay";
const DEFAULT_PORT: u16 = 1423;

pub fn init<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let pending = relay::PendingResults::default();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(tauri::generate_handler![commands::relay_result])
        .setup(move |app, _api| {
            let port = std::env::var("RELAY_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_PORT);

            app.manage(pending.clone());
            start_server(app.clone(), port, pending.clone());
            Ok(())
        })
        .build()
}

fn start_server<R: Runtime>(app: AppHandle<R>, port: u16, pending: relay::PendingResults) {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    tauri::async_runtime::spawn(async move {
        if let Err(e) = server::run(app, addr, pending).await {
            tracing::error!("[relay] server failed: {e}");
        }
    });
}
