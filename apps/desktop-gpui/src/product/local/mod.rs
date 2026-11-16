pub mod calendar;
pub mod data;
pub mod developers;
pub mod export;
pub mod models;
pub mod onboarding;
pub mod storage;
#[cfg(test)]
mod tests;
mod transcript;
pub mod view;

use std::{path::PathBuf, sync::Arc};

use anlg_calendar::CalendarProviderType;
use desktop_runtime::{Result, RuntimeHandle, ServiceError};
use futures::future::BoxFuture;

use super::services::{
    Action, Field, Mutation, Operation, Outcome, Panel, ProductServices, Request, Row, Surface,
};
use crate::platform::permissions::{NativePermissions, Permission, Status};

pub fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Failed(error.to_string().into())
}

pub trait HostEffects: Send + Sync {
    fn flush_drafts(&self) -> BoxFuture<'static, Result<()>>;
    fn pause_writers(&self) -> BoxFuture<'static, Result<()>>;
    fn resume_writers(&self) -> BoxFuture<'static, Result<()>>;
    fn relaunch(&self) -> BoxFuture<'static, Result<()>>;
    fn use_model(
        &self,
        model: anlg_local_model::LocalModel,
        endpoint: String,
        native: Option<models::NativeEngine>,
    ) -> BoxFuture<'static, Result<()>>;
    fn stop_model(&self) -> BoxFuture<'static, Result<()>>;
}

#[derive(Clone)]
pub struct LocalServices {
    pub runtime: RuntimeHandle,
    pub cloud: Arc<dyn ProductServices>,
    pub host: Arc<dyn HostEffects>,
    pub permissions: NativePermissions,
    pub models: Arc<models::ModelService>,
    pub calendar: calendar::CalendarService,
    pub developers: developers::DeveloperTools,
    pub storage_root: PathBuf,
    pub storage_pointer: PathBuf,
}

impl ProductServices for LocalServices {
    fn load(&self, surface: Surface, request: Request) -> BoxFuture<'static, Result<Panel>> {
        if !owned(surface) {
            return self.cloud.load(surface, request);
        }
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(move |_| async move { this.load_local(surface, request).await })?
                .receive()
                .await
        })
    }

    fn perform(
        &self,
        surface: Surface,
        request: Request,
        mutation: Mutation,
    ) -> BoxFuture<'static, Result<Outcome>> {
        if !owned(surface)
            || matches!(
                mutation.operation.action,
                Action::ConnectImport
                    | Action::SyncImport
                    | Action::EnableCloudApi
                    | Action::BackfillCloudApi
                    | Action::CreateApiKey
                    | Action::CopyApiKey
                    | Action::RevokeApiKey
                    | Action::CreateWebhook
                    | Action::DeleteWebhook
                    | Action::SetWebhookActive
                    | Action::TestWebhook
            )
        {
            return self.cloud.perform(surface, request, mutation);
        }
        let this = self.clone();
        Box::pin(async move {
            this.runtime
                .clone()
                .service(
                    move |_| async move { this.perform_local(surface, request, mutation).await },
                )?
                .receive()
                .await
        })
    }
}

fn owned(surface: Surface) -> bool {
    matches!(
        surface,
        Surface::Onboarding
            | Surface::Permissions
            | Surface::Imports
            | Surface::Exports
            | Surface::Models
            | Surface::Calendar
            | Surface::Developers
            | Surface::Storage
    )
}

impl LocalServices {
    async fn load_local(&self, surface: Surface, request: Request) -> Result<Panel> {
        if request.cancel.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        let mut rows = Vec::new();
        let mut actions = Vec::new();
        let mut fields = Vec::new();
        let mut permissions_ready = false;
        match surface {
            Surface::Permissions => {
                let permissions = self.permissions.clone();
                let states = tokio::task::spawn_blocking(move || permission_rows(&permissions))
                    .await
                    .map_err(failure)?;
                permissions_ready = states
                    .iter()
                    .filter(|(id, _, _)| id != "calendar")
                    .all(|(_, _, status)| matches!(status, Status::Granted | Status::NotRequired));
                for (id, title, status) in states {
                    let detail = match status {
                        Status::Granted => "Allowed".into(),
                        Status::NotRequired => "Not required".into(),
                        Status::Denied(error) => error,
                    };
                    rows.push(row(&id, title, &detail));
                    actions.push(operation(
                        &format!("{id}:request"),
                        "Allow access",
                        Action::RequestPermission,
                        Some(&id),
                    ));
                    actions.push(operation(
                        &format!("{id}:settings"),
                        "Open System Settings",
                        Action::OpenPermissionSettings,
                        Some(&id),
                    ));
                }
            }
            Surface::Onboarding => {
                let complete = data::setting_value(&self.runtime, "onboarding_needed".into())
                    .await?
                    == Some(serde_json::Value::Bool(false));
                rows.push(row("welcome", "Welcome to Anarlog", if complete { "Setup complete. Your welcome meeting is saved." } else { "Your private AI notepad for meetings. Set up permissions, connect your calendar, and bring your notes." }));
                actions.push(operation(
                    "complete",
                    "Open Anarlog",
                    Action::CompleteOnboarding,
                    None,
                ));
            }
            Surface::Imports => {
                rows.push(row("files", "Import from files", "Choose meeting JSON, CSV, Markdown, text, SRT or VTT. Up to 20 MB per file and 100 MB total. All selected meetings are committed together."));
                actions.push(operation(
                    "import",
                    "Choose files",
                    Action::ImportFiles,
                    None,
                ));
            }
            Surface::Exports => {
                rows.push(row("formats", "Export meeting", "Export your summary, memo and transcript. Canonical JSON preserves unknown document nodes and original transcript data."));
                fields.extend([
                    field("format", "Format", "pdf"),
                    field("memo", "Include memo", "false"),
                    field("summary", "Include summary", "true"),
                    field("transcript", "Include transcript", "false"),
                ]);
                actions.push(operation("export", "Export", Action::Export, None));
            }
            Surface::Models => {
                for model in models::ModelService::registry() {
                    let state = self.models.state(model.clone()).await?;
                    let id = model.cli_name();
                    let progress = match &state.progress {
                        Some(anlg_model_downloader::DownloadStatus::Downloading(percent)) => {
                            format!(" · Downloading {percent}%")
                        }
                        Some(anlg_model_downloader::DownloadStatus::Failed(error)) => {
                            format!(" · {error}")
                        }
                        _ => String::new(),
                    };
                    rows.push(row(
                        id,
                        &model.display_name(),
                        &format!(
                            "{} · {}{}{}",
                            model.description(),
                            if !model.is_available_on_current_platform() {
                                "Unavailable on this platform"
                            } else if state.installed {
                                "Downloaded"
                            } else {
                                "Not downloaded"
                            },
                            if state.running { " · Running" } else { "" },
                            progress
                        ),
                    ));
                    let mut available = Vec::new();
                    if state.running {
                        available.push(("stop", "Stop", Action::StopModel));
                    } else if state.installed {
                        available.push(("start", "Start", Action::StartModel));
                        available.push(("delete", "Delete", Action::DeleteModel));
                    } else {
                        available.push(("download", "Download", Action::DownloadModel));
                        if matches!(
                            state.progress,
                            Some(anlg_model_downloader::DownloadStatus::Downloading(_))
                        ) {
                            available.push(("cancel", "Cancel download", Action::CancelDownload));
                        }
                    }
                    for (suffix, label, action) in available {
                        let mut op = operation(&format!("{id}:{suffix}"), label, action, Some(id));
                        if !model.is_available_on_current_platform() {
                            op.disabled_reason = Some("Unavailable on this platform".into());
                        } else if matches!(&model,anlg_local_model::LocalModel::Soniqo(model) if !model.supports_live())
                            && op.action == Action::StartModel
                        {
                            op.disabled_reason =
                                Some("Batch model loads when transcribing an audio file".into());
                        }
                        actions.push(op);
                    }
                }
            }
            Surface::Calendar => {
                let offset = self.calendar.page(&request.scope)?;
                let mut discovered = Vec::new();
                let connections = match self.calendar.connections(request.cancel.clone()).await {
                    Ok(connections) => connections,
                    Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
                    Err(error) => {
                        rows.push(row(
                            "connection-status",
                            "Calendar account",
                            &error.to_string(),
                        ));
                        Vec::new()
                    }
                };
                for provider in anlg_calendar::available_providers() {
                    let id = provider_id(provider);
                    rows.push(row(
                        id,
                        match provider {
                            CalendarProviderType::Apple => "Apple Calendar",
                            CalendarProviderType::Google => "Google Calendar",
                            CalendarProviderType::Outlook => "Outlook Calendar",
                        },
                        "Connect your calendar to see upcoming meetings",
                    ));
                    actions.push(operation(
                        &format!("{id}:connect"),
                        "Connect",
                        Action::ConnectCalendar,
                        Some(id),
                    ));
                    let ids = if id == "apple" {
                        vec!["apple".into()]
                    } else {
                        connections
                            .iter()
                            .find(|entry| entry.provider == provider)
                            .map(|entry| entry.connection_ids.clone())
                            .unwrap_or_default()
                    };
                    for connection in ids {
                        match self
                            .calendar
                            .discover(
                                &self.runtime,
                                provider,
                                connection.clone(),
                                request.cancel.clone(),
                            )
                            .await
                        {
                            Ok(calendars) => {
                                if calendars.is_empty() && id != "apple" {
                                    discovered.push(serde_json::json!({"id":format!("{id}:{connection}"),"name":format!("{id} account"),"connection_only":true,"provider":id,"connection":connection}));
                                }
                                discovered.extend(calendars);
                            }
                            Err(ServiceError::Cancelled) => return Err(ServiceError::Cancelled),
                            Err(error) => rows.push(row(
                                &format!("{id}:{connection}"),
                                "Calendar connection",
                                &error.to_string(),
                            )),
                        }
                    }
                }
                discovered.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
                let offset = offset.min(discovered.len().saturating_sub(1) / 24 * 24);
                let mut connections = std::collections::BTreeSet::new();
                for calendar in discovered.iter().skip(offset).take(24) {
                    let key = calendar["id"].as_str().ok_or(ServiceError::Conflict)?;
                    let enabled = calendar["enabled"].as_bool().unwrap_or(false);
                    if enabled {
                        self.calendar
                            .refresh(&self.runtime, key.into(), request.cancel.clone())
                            .await?;
                    }
                    rows.push(row(
                        key,
                        calendar["name"].as_str().unwrap_or("Untitled"),
                        if enabled { "Enabled" } else { "Disabled" },
                    ));
                    if calendar["connection_only"] != true {
                        actions.push(operation(
                            key,
                            if enabled { "Disable" } else { "Enable" },
                            Action::SelectProvider,
                            Some(key),
                        ));
                    }
                    if calendar["provider"] != "apple" {
                        connections.insert(format!(
                            "{}:{}",
                            calendar["provider"].as_str().unwrap_or(""),
                            calendar["connection"].as_str().unwrap_or("")
                        ));
                    }
                }
                for connection in connections {
                    actions.push(operation(
                        &format!("disconnect:{connection}"),
                        "Disconnect account",
                        Action::DisconnectCalendar,
                        Some(&connection),
                    ));
                }
                if offset > 0 {
                    actions.push(operation(
                        "previous-calendars",
                        "Previous",
                        Action::NextPage,
                        Some(&offset.saturating_sub(24).to_string()),
                    ));
                }
                if offset + 24 < discovered.len() {
                    actions.push(operation(
                        "next-calendars",
                        "Next",
                        Action::NextPage,
                        Some(&(offset + 24).to_string()),
                    ));
                }
            }
            Surface::Developers => {
                let developer = self.developers.clone();
                let installed = tokio::task::spawn_blocking(move || developer.cli_installed())
                    .await
                    .map_err(failure)??;
                rows.push(row(
                    "cli",
                    "Anarlog CLI",
                    if installed {
                        "Installed"
                    } else {
                        "Use Anarlog from your terminal"
                    },
                ));
                rows.push(row(
                    "mcp",
                    "MCP",
                    "Connect Claude Desktop or another MCP client to your local notes",
                ));
                rows.push(row(
                    "skills",
                    "Agent skills",
                    "Teach your coding agent how to use Anarlog",
                ));
                actions.extend([
                    operation("install-cli", "Install", Action::InstallCli, None),
                    operation(
                        "install-mcp",
                        "Install MCP config",
                        Action::InstallMcp,
                        None,
                    ),
                ]);
                for agent in ["claude_code", "codex", "cursor", "opencode"] {
                    actions.push(operation(
                        &format!("skill:{agent}"),
                        agent,
                        Action::InstallSkills,
                        Some(agent),
                    ));
                }
            }
            Surface::Storage => {
                rows.push(row(
                    "storage",
                    "Where your notes and recordings are stored",
                    &self.storage_root.display().to_string(),
                ));
                rows.push(row("safe-copy","Change storage location","Copies and validates your data before switching. Original files remain available for rollback."));
                actions.push(operation("move", "Change", Action::MoveStorage, None));
            }
            _ => return Err(ServiceError::Conflict),
        }
        Ok(Panel {
            scope: request.scope,
            status: "".into(),
            rows: rows.into(),
            fields: fields.into(),
            operations: actions.into(),
            permissions_ready,
        })
    }

    async fn perform_local(
        &self,
        surface: Surface,
        request: Request,
        mutation: Mutation,
    ) -> Result<Outcome> {
        if request.cancel.is_cancelled() {
            return Err(ServiceError::Cancelled);
        }
        let target = mutation.operation.target_id.as_deref().unwrap_or("");
        match (surface, &mutation.operation.action) {
            (Surface::Onboarding, Action::CompleteOnboarding) => {
                return data::complete_onboarding(&self.runtime)
                    .await
                    .map(Outcome::OpenSession);
            }
            (Surface::Permissions, Action::RequestPermission | Action::OpenPermissionSettings) => {
                let permission = permission(target)?;
                let native = self.permissions.clone();
                let settings = mutation.operation.action == Action::OpenPermissionSettings;
                tokio::task::spawn_blocking(move || {
                    if settings {
                        return native.open_settings(permission);
                    }
                    match native.request(permission) {
                        Status::Denied(error) => Err(failure(error)),
                        _ => Ok(()),
                    }
                })
                .await
                .map_err(failure)??;
            }
            (Surface::Imports, Action::ImportFiles | Action::RetryImport) => {
                let files = rfd::AsyncFileDialog::new()
                    .set_title("Import meetings")
                    .add_filter(
                        "Meeting exports",
                        &["json", "csv", "md", "markdown", "txt", "srt", "vtt"],
                    )
                    .pick_files()
                    .await
                    .ok_or(ServiceError::Cancelled)?;
                let ids = data::import_files(
                    &self.runtime,
                    files.iter().map(|file| file.path().to_owned()).collect(),
                    request.cancel,
                )
                .await?;
                if let Some(id) = ids.into_iter().next() {
                    return Ok(Outcome::OpenSession(id));
                }
            }
            (Surface::Exports, Action::Export) => {
                self.host.flush_drafts().await?;
                let id = request
                    .scope
                    .session_id
                    .ok_or_else(|| failure("Open a meeting before exporting"))?;
                let options = export::ExportOptions {
                    format: match field_value(&mutation, "format") {
                        "pdf" => export::Format::Pdf,
                        "md" => export::Format::Markdown,
                        "txt" => export::Format::Text,
                        "org" => export::Format::Org,
                        "json" => export::Format::Canonical,
                        _ => return Err(failure("Unknown export format")),
                    },
                    memo: field_value(&mutation, "memo") == "true",
                    summary: field_value(&mutation, "summary") != "false",
                    transcript: field_value(&mutation, "transcript") == "true",
                    summary_id: None,
                };
                let meeting = data::snapshot(&self.runtime, id).await?;
                let file = rfd::AsyncFileDialog::new()
                    .set_title("Export meeting")
                    .set_file_name(format!(
                        "{}.{}",
                        anlg_meeting_import::safe_file_component(meeting.title()),
                        options.format.extension()
                    ))
                    .add_filter("Export", &[options.format.extension()])
                    .save_file()
                    .await
                    .ok_or(ServiceError::Cancelled)?;
                if request.cancel.is_cancelled() {
                    return Err(ServiceError::Cancelled);
                }
                tokio::task::spawn_blocking(move || {
                    export::export(&meeting, &options, file.path())
                })
                .await
                .map_err(failure)??;
            }
            (Surface::Models, action) => {
                let model = models::ModelService::resolve(target)?;
                match action {
                    Action::DownloadModel | Action::RetryDownload => {
                        self.models.download(model).await?
                    }
                    Action::CancelDownload => self.models.cancel(model).await?,
                    Action::DeleteModel => self.models.delete(model).await?,
                    Action::StartModel => {
                        if !matches!(model, anlg_local_model::LocalModel::GgufLlm(_)) {
                            self.host.stop_model().await?;
                        }
                        self.models.stop_kind(&model).await?;
                        let endpoint = self.models.start(model.clone(), request.cancel).await?;
                        let native = if matches!(model, anlg_local_model::LocalModel::GgufLlm(_)) {
                            None
                        } else {
                            self.models.take_native_session().await
                        };
                        if let Err(error) =
                            self.host.use_model(model.clone(), endpoint, native).await
                        {
                            self.models.stop_kind(&model).await?;
                            return Err(error);
                        }
                    }
                    Action::StopModel => {
                        if !matches!(model, anlg_local_model::LocalModel::GgufLlm(_)) {
                            self.host.stop_model().await?;
                        }
                        self.models.stop_kind(&model).await?;
                    }
                    _ => return Err(ServiceError::Conflict),
                }
            }
            (Surface::Calendar, Action::ConnectCalendar) => {
                let provider = parse_provider(target)?;
                if provider == CalendarProviderType::Apple {
                    let permissions = self.permissions.clone();
                    let status = tokio::task::spawn_blocking(move || {
                        permissions.request(Permission::Calendar)
                    })
                    .await
                    .map_err(failure)?;
                    if let Status::Denied(error) = status {
                        return Err(failure(error));
                    }
                } else {
                    self.calendar.auth.connect(provider, request.cancel).await?;
                }
            }
            (Surface::Calendar, Action::DisconnectCalendar) => {
                let (provider, connection) = target
                    .split_once(':')
                    .ok_or_else(|| failure("Missing calendar connection"))?;
                self.calendar
                    .auth
                    .disconnect(parse_provider(provider)?, connection.into(), request.cancel)
                    .await?;
                let provider = provider.to_owned();
                let connection = connection.to_owned();
                self.runtime.submit(move |services| async move {
                    services.executor.execute_transaction(vec![
                        anlg_db_execute::TransactionStatement {sql:"UPDATE events SET deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE calendar_id IN (SELECT id FROM calendars WHERE provider=? AND connection_id=?) AND deleted_at IS NULL".into(),params:vec![serde_json::json!(provider),serde_json::json!(connection)],expected_rows_affected:None},
                        anlg_db_execute::TransactionStatement {sql:"UPDATE calendars SET enabled=0,deleted_at=strftime('%Y-%m-%dT%H:%M:%SZ','now'),updated_at=strftime('%Y-%m-%dT%H:%M:%SZ','now') WHERE provider=? AND connection_id=?".into(),params:vec![serde_json::json!(provider),serde_json::json!(connection)],expected_rows_affected:None}
                    ]).await.map_err(failure)?;
                    Ok(())
                })?.receive().await?;
            }
            (Surface::Calendar, Action::SelectProvider) => {
                self.calendar
                    .toggle(&self.runtime, target.into(), request.cancel)
                    .await?;
            }
            (Surface::Calendar, Action::NextPage) => {
                self.calendar
                    .set_page(request.scope.clone(), target.parse().map_err(failure)?)?;
            }
            (
                Surface::Developers,
                Action::InstallCli | Action::InstallMcp | Action::InstallSkills,
            ) => {
                let tools = self.developers.clone();
                match mutation.operation.action {
                    Action::InstallCli => tokio::task::spawn_blocking(move || tools.install_cli())
                        .await
                        .map_err(failure)??,
                    Action::InstallSkills => {
                        let agent = target.to_owned();
                        tokio::task::spawn_blocking(move || tools.install_skills(&agent))
                            .await
                            .map_err(failure)??;
                    }
                    Action::InstallMcp => {
                        let file = rfd::AsyncFileDialog::new()
                            .set_title("Choose MCP client JSON configuration")
                            .add_filter("JSON", &["json"])
                            .pick_file()
                            .await
                            .ok_or(ServiceError::Cancelled)?;
                        tokio::task::spawn_blocking(move || tools.install_mcp(file.path()))
                            .await
                            .map_err(failure)??;
                    }
                    _ => unreachable!(),
                }
            }
            (Surface::Storage, Action::MoveStorage) => {
                let target = rfd::AsyncFileDialog::new()
                    .set_title("Choose empty storage location")
                    .pick_folder()
                    .await
                    .ok_or(ServiceError::Cancelled)?
                    .path()
                    .to_owned();
                self.host.flush_drafts().await?;
                self.host.pause_writers().await?;
                let root = self.storage_root.clone();
                let pointer = self.storage_pointer.clone();
                let cancel = request.cancel;
                let result = tokio::task::spawn_blocking(move || {
                    let prepared = storage::PreparedMove::prepare(&root, &target, &cancel)?;
                    match prepared.commit(&pointer, &cancel) {
                        Ok(committed) => Ok(committed),
                        Err(error) => {
                            prepared.abort()?;
                            Err(error)
                        }
                    }
                })
                .await
                .map_err(failure)
                .and_then(|result| result);
                let committed = match result {
                    Ok(committed) => committed,
                    Err(error) => {
                        self.host.resume_writers().await?;
                        return Err(error);
                    }
                };
                if let Err(error) = self.host.relaunch().await {
                    tokio::task::spawn_blocking(move || committed.rollback())
                        .await
                        .map_err(failure)??;
                    self.host.resume_writers().await?;
                    return Err(error);
                }
            }
            _ => return Err(ServiceError::Conflict),
        }
        Ok(Outcome::Refresh)
    }
}

fn field_value<'a>(mutation: &'a Mutation, key: &str) -> &'a str {
    mutation
        .fields
        .iter()
        .find(|(id, _)| id.as_ref() == key)
        .map(|(_, value)| value.as_ref())
        .unwrap_or("")
}

pub fn operation(id: &str, label: &str, action: Action, target: Option<&str>) -> Operation {
    let confirmation = action.requires_confirmation().then(|| {
        format!("{label}? This changes your local installation or stored settings.").into()
    });
    Operation {
        id: id.into(),
        label: label.into(),
        action,
        target_id: target.map(Into::into),
        disabled_reason: None,
        confirmation,
    }
}

fn row(id: &str, title: &str, detail: &str) -> Row {
    Row {
        id: id.into(),
        title: title.into(),
        detail: detail.into(),
    }
}
fn field(id: &str, label: &str, value: &str) -> Field {
    Field {
        id: id.into(),
        label: label.into(),
        value: value.into(),
    }
}

fn permission(id: &str) -> Result<Permission> {
    match id {
        "microphone" => Ok(Permission::Microphone),
        "system_audio" => Ok(Permission::SystemAudio),
        "accessibility" => Ok(Permission::Accessibility),
        "calendar" => Ok(Permission::Calendar),
        _ => Err(ServiceError::Conflict),
    }
}

fn permission_rows(permissions: &NativePermissions) -> Vec<(String, &'static str, Status)> {
    [
        ("microphone", "Microphone", Permission::Microphone),
        ("system_audio", "System audio", Permission::SystemAudio),
        ("accessibility", "Accessibility", Permission::Accessibility),
        ("calendar", "Calendar", Permission::Calendar),
    ]
    .into_iter()
    .map(|(id, title, permission)| (id.into(), title, permissions.check(permission)))
    .collect()
}

fn provider_id(provider: CalendarProviderType) -> &'static str {
    match provider {
        CalendarProviderType::Apple => "apple",
        CalendarProviderType::Google => "google",
        CalendarProviderType::Outlook => "outlook",
    }
}

fn parse_provider(value: &str) -> Result<CalendarProviderType> {
    match value {
        "apple" => Ok(CalendarProviderType::Apple),
        "google" => Ok(CalendarProviderType::Google),
        "outlook" => Ok(CalendarProviderType::Outlook),
        _ => Err(failure("Unknown calendar provider")),
    }
}
