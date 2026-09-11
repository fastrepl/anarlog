mod commands;
mod import;
mod runtime;

pub use anlg_desktop_db_runtime::{Error, Result};
pub use runtime::{open_app_db, open_app_db_unmigrated};
use tauri::Manager;

const PLUGIN_NAME: &str = "db";

pub type ManagedState = std::sync::Arc<runtime::PluginDbRuntime>;

pub use anlg_desktop_db_runtime::{
    CloudsyncE2eeWitness, CloudsyncTokenConfigurationResult, CloudsyncWorkspaceKeyGrant,
    CloudsyncWorkspaceProjection, CloudsyncWorkspaceProjectionEntry, E2eeDeviceEnrollmentPackage,
    E2eeDeviceIdentity, E2eeIdentityStatus, E2eeRecoveryKeyIdentity, ExecuteProxyResult,
    LegacyCleanupResult, LegacyCleanupStatus, LegacyImportItemReport, LegacyImportReport,
    LegacyImportRun, LegacyImportTargetReport, QueryEvent, SealedWorkspaceE2eeKey,
    SessionIngestApplyResult, StartupPhase, StartupStatus, StorageMigrationState,
    TransactionStatement, WorkspaceE2eeKeyGrantUpload, WorkspaceE2eeKeyRecipient,
};

fn make_specta_builder<R: tauri::Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .plugin_name(PLUGIN_NAME)
        .commands(tauri_specta::collect_commands![
            commands::list_meetings,
            commands::get_meeting,
            commands::get_meeting_transcript,
            commands::get_recurring_meeting_history,
            commands::execute,
            commands::execute_transaction,
            commands::execute_proxy,
            commands::get_legacy_import_report,
            commands::get_legacy_cleanup_status,
            commands::cleanup_legacy_files,
            commands::run_legacy_import,
            commands::apply_session_ingest,
            commands::get_e2ee_identity_status<tauri::Wry>,
            commands::inspect_e2ee_recovery_key,
            commands::create_e2ee_identity<tauri::Wry>,
            commands::import_e2ee_identity<tauri::Wry>,
            commands::get_or_create_e2ee_device_identity<tauri::Wry>,
            commands::seal_e2ee_recovery_key_for_device<tauri::Wry>,
            commands::seal_workspace_e2ee_key_for_recipients<tauri::Wry>,
            commands::import_e2ee_device_enrollment<tauri::Wry>,
            commands::subscribe,
            commands::unsubscribe,
            commands::configure_cloudsync,
            commands::bind_cloudsync_account,
            commands::configure_cloudsync_token<tauri::Wry>,
            commands::configure_e2ee_replica<tauri::Wry>,
            commands::start_cloudsync,
            commands::stop_cloudsync,
            commands::suspend_cloudsync,
            commands::suspend_cloudsync_for_sign_out,
            commands::suspend_cloudsync_after_auth_loss,
            commands::get_cloudsync_status,
            commands::sync_cloudsync_now,
            commands::begin_cloudsync_activity,
            commands::end_cloudsync_activity,
            commands::get_startup_status,
            commands::wait_until_ready,
        ])
        .error_handling(tauri_specta::ErrorHandlingMode::Result)
}

async fn bootstrap_app_database<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    db: std::sync::Arc<anlg_db_core::Db>,
    runtime: &runtime::PluginDbRuntime,
    startup_config: Option<anlg_db_core::CloudsyncRuntimeConfig>,
) -> std::result::Result<(), String> {
    runtime
        .ensure_app_schema()
        .await
        .map_err(|error| error.to_string())?;
    if import::legacy_import_attempt_required(db.pool())
        .await
        .map_err(|error| error.to_string())?
    {
        runtime.set_startup_status_if_running(StartupStatus::for_phase(
            StartupPhase::ImportingLegacyData,
        ));
    }
    import::import_legacy_data(&app, db.pool())
        .await
        .map_err(|error| error.to_string())?;
    if let Some(config) = startup_config {
        runtime.set_startup_status_if_running(StartupStatus::for_phase(
            StartupPhase::ConfiguringCloudsync,
        ));
        let migration_ready = import::legacy_migration_ready(db.pool())
            .await
            .map_err(|error| error.to_string())?;
        if !migration_ready {
            tracing::warn!(
                "startup CloudSync configuration skipped until legacy migration is ready"
            );
        } else if let Err(error) = db.cloudsync_configure(config).await {
            tracing::warn!(%error, "failed to configure startup cloudsync");
        } else {
            let sync_db = std::sync::Arc::clone(&db);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = sync_db.cloudsync_start().await {
                    tracing::warn!(%error, "failed to start cloudsync");
                    return;
                }
                if let Err(error) = sync_db.cloudsync_trigger_sync().await {
                    tracing::warn!(%error, "initial cloudsync failed");
                }
            });
        }
    }
    Ok(())
}

fn ensure_db_sync_runtime() -> tokio::runtime::Handle {
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        return handle;
    }
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    tauri::async_runtime::spawn(async move {
        let _ = tx.send(tokio::runtime::Handle::current());
    });
    rx.recv_timeout(std::time::Duration::from_secs(2))
        .unwrap_or_else(|_| tauri::async_runtime::handle().inner().clone())
}

pub fn init<R: tauri::Runtime>(
    db: std::sync::Arc<anlg_db_core::Db>,
) -> tauri::plugin::TauriPlugin<R> {
    init_with_cloudsync(db, None)
}

pub fn init_with_cloudsync<R: tauri::Runtime>(
    db: std::sync::Arc<anlg_db_core::Db>,
    startup_config: Option<anlg_db_core::CloudsyncRuntimeConfig>,
) -> tauri::plugin::TauriPlugin<R> {
    let specta_builder = make_specta_builder();

    tauri::plugin::Builder::new(PLUGIN_NAME)
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app, _| {
            let handle = ensure_db_sync_runtime();
            let runtime = std::sync::Arc::new(runtime::PluginDbRuntime::new(
                std::sync::Arc::clone(&db),
                handle,
            ));
            let startup_db = std::sync::Arc::clone(&db);
            let startup_runtime = std::sync::Arc::clone(&runtime);
            let startup_app = app.app_handle().clone();
            let startup_config = startup_config.clone();
            tauri::async_runtime::spawn(async move {
                let result = bootstrap_app_database(
                    startup_app,
                    startup_db,
                    &startup_runtime,
                    startup_config,
                )
                .await;
                startup_runtime.finish_startup(result);
            });
            app.manage(runtime);
            Ok(())
        })
        .on_event(|app, event| {
            if let tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::Focused(true),
                ..
            } = event
                && let Some(runtime) = app.try_state::<ManagedState>()
            {
                runtime.nudge_cloudsync_on_focus();
            }
        })
        .build()
}

#[cfg(test)]
mod tests;
