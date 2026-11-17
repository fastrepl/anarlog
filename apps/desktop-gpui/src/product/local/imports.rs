//! Shipping's meeting imports (`imports/{providers,detection,queries}.ts`): the provider
//! catalog, installed-app detection, and provider-scoped imports recorded in
//! `migration_import_runs` so repeated syncs and files are recognized.

use std::{io::Read, path::PathBuf, sync::Arc};

use anlg_db_execute::TransactionStatement;
use desktop_runtime::{CancellationToken, Result, RuntimeHandle};
use gpui::{App, Global};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{data, failure};

const IMPORTER_VERSION: i64 = 2;
pub const EXTENSIONS: &[&str] = &["csv", "json", "md", "markdown", "srt", "txt", "vtt"];
pub const DOCUMENTATION_URL: &str = "https://docs.anarlog.so/imports";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direct {
    Cli,
    McpOauth,
    NangoOauth,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub direct: Option<Direct>,
    pub nango: Option<&'static str>,
    native_names: &'static [&'static str],
    bundle_ids: &'static [&'static str],
    always: bool,
}

impl Provider {
    /// Imported through a connection rather than only from export files.
    pub fn connected(&self) -> bool {
        self.direct.is_some()
    }

    pub fn icon(&self) -> Option<&'static str> {
        match self.id {
            "google-meet" => Some("product/GoogleMeet.png"),
            _ => None,
        }
    }
}

const fn provider(
    id: &'static str,
    name: &'static str,
    direct: Option<Direct>,
    native_names: &'static [&'static str],
    bundle_ids: &'static [&'static str],
) -> Provider {
    Provider {
        id,
        name,
        direct,
        nango: None,
        native_names,
        bundle_ids,
        always: false,
    }
}

const fn nango(
    id: &'static str,
    name: &'static str,
    native_names: &'static [&'static str],
    bundle_ids: &'static [&'static str],
) -> Provider {
    Provider {
        id,
        name,
        direct: Some(Direct::NangoOauth),
        nango: Some(id),
        native_names,
        bundle_ids,
        always: false,
    }
}

const MCP: Option<Direct> = Some(Direct::McpOauth);

/// `MEETING_IMPORT_PROVIDERS`.
pub const PROVIDERS: &[Provider] = &[
    provider(
        "granola",
        "Granola",
        MCP,
        &["Granola"],
        &["com.granola.app", "com.getgranola.app"],
    ),
    provider("circleback", "Circleback", MCP, &["Circleback"], &[]),
    provider(
        "fireflies",
        "Fireflies.ai",
        MCP,
        &["Fireflies", "Fireflies.ai"],
        &[],
    ),
    provider("krisp", "Krisp", MCP, &["Krisp"], &["ai.krisp.krispMac"]),
    nango("fathom", "Fathom", &["Fathom"], &["Fathom"]),
    provider("read-ai", "Read AI", MCP, &["Read AI"], &[]),
    nango(
        "notion",
        "Notion AI Meeting Notes",
        &["Notion"],
        &["notion.id", "notion"],
    ),
    provider("fellow", "Fellow", MCP, &["Fellow"], &[]),
    provider("tactiq", "Tactiq", MCP, &[], &[]),
    provider("grain", "Grain", None, &["Grain"], &[]),
    provider("otter", "Otter.ai", None, &["Otter", "Otter.ai"], &[]),
    provider("tldv", "tl;dv", None, &["tl;dv", "tldv"], &[]),
    provider("meetgeek", "MeetGeek", None, &["MeetGeek"], &[]),
    provider("avoma", "Avoma", None, &[], &[]),
    provider("gong", "Gong", None, &[], &[]),
    provider(
        "clari-copilot",
        "Clari Copilot",
        None,
        &["Clari Copilot", "Wingman"],
        &[],
    ),
    provider(
        "jiminny",
        "Jiminny",
        MCP,
        &["Jiminny", "Jiminny Sidekick"],
        &[],
    ),
    provider("supernormal", "Supernormal", None, &["Supernormal"], &[]),
    provider(
        "plaud",
        "Plaud",
        Some(Direct::Cli),
        &["Plaud", "Plaud Desktop"],
        &["ai.plaud.desktop.plaud"],
    ),
    provider(
        "pocket",
        "Pocket",
        MCP,
        &["Pocket", "Pocket Desktop", "Pocket AI"],
        &["com.openvisionengineering.pocket-desktop-app"],
    ),
    nango(
        "zoom",
        "Zoom",
        &["zoom.us", "Zoom", "Zoom Workplace"],
        &["us.zoom.xos"],
    ),
    nango(
        "microsoft-teams",
        "Microsoft Teams",
        &["Microsoft Teams", "Microsoft Teams (work or school)"],
        &["com.microsoft.teams", "com.microsoft.teams2"],
    ),
    Provider {
        always: true,
        ..nango("google-meet", "Google Meet", &["Google Meet"], &[])
    },
    nango(
        "webex",
        "Webex",
        &["Webex", "Cisco Webex Meetings"],
        &["com.cisco.webex", "com.webex"],
    ),
    provider("sembly", "Sembly", None, &["Sembly"], &[]),
    provider("notta", "Notta", None, &["Notta"], &[]),
    provider("bluedot", "Bluedot", None, &[], &[]),
    provider("jamie", "Jamie", None, &["Jamie"], &["com.jamie.app"]),
    provider(
        "chatgpt-record",
        "ChatGPT Record",
        None,
        &["ChatGPT"],
        &["com.openai.chat", "com.openai.chatgpt"],
    ),
    provider(
        "slack-huddles",
        "Slack Huddles",
        None,
        &["Slack"],
        &["com.tinyspeck.slackmacgap"],
    ),
    provider(
        "limitless",
        "Limitless / Rewind",
        None,
        &["Limitless", "Rewind"],
        &["ai.limitless.desktop", "com.memoryvault.MemoryVault"],
    ),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
}

pub trait InstalledApps: Send + Sync {
    fn list(&self) -> Result<Vec<InstalledApp>>;
}

/// Replaces the system app list (tests and screenshots).
#[derive(Clone)]
pub struct InstalledAppsOverride(pub Arc<dyn InstalledApps>);

impl Global for InstalledAppsOverride {}

pub fn installed_apps(cx: &App) -> Arc<dyn InstalledApps> {
    cx.try_global::<InstalledAppsOverride>()
        .map(|installed| installed.0.clone())
        .unwrap_or_else(|| Arc::new(SystemApps))
}

/// Application bundles in `/Applications` and `~/Applications`, named by their bundle
/// directory; bundle identifiers are not read.
pub struct SystemApps;

impl InstalledApps for SystemApps {
    fn list(&self) -> Result<Vec<InstalledApp>> {
        if !cfg!(target_os = "macos") {
            return Ok(Vec::new());
        }
        let mut roots = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            roots.push(PathBuf::from(home).join("Applications"));
        }
        let mut apps = Vec::new();
        while let Some(directory) = roots.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                match path.extension().and_then(|value| value.to_str()) {
                    Some("app") => {
                        if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
                            apps.push(InstalledApp {
                                id: name.to_owned(),
                                name: name.to_owned(),
                            });
                        }
                    }
                    _ => roots.push(path),
                }
            }
        }
        Ok(apps)
    }
}

fn normalize(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let lower = lower.strip_suffix(".app").unwrap_or(&lower);
    lower
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `detectMeetingImportProviders` plus the screen's ordering: connected imports first, then
/// file-only providers, each by name.
pub fn detect(installed: &[InstalledApp]) -> Vec<&'static Provider> {
    let installed = installed
        .iter()
        .map(|app| (app.id.to_lowercase(), normalize(&app.name)))
        .collect::<Vec<_>>();
    let mut providers = PROVIDERS
        .iter()
        .filter(|provider| {
            provider.always
                || installed.iter().any(|(id, name)| {
                    provider
                        .native_names
                        .iter()
                        .any(|native| *name == normalize(native))
                        || provider
                            .bundle_ids
                            .iter()
                            .any(|bundle| *id == bundle.to_lowercase())
                })
        })
        .collect::<Vec<_>>();
    providers.sort_by_key(|provider| (!provider.connected(), provider.name.to_lowercase()));
    providers
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportResult {
    pub discovered: i64,
    pub imported: i64,
    pub matched: i64,
    pub conflicts: i64,
    pub errors: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportRun {
    pub provider_id: String,
    pub result: ImportResult,
}

#[derive(Clone, Debug)]
pub struct TextFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Connection,
    Export,
}

/// `useMeetingImportHistory`: the latest runs, newest first.
pub async fn history(runtime: &RuntimeHandle) -> Result<Vec<ImportRun>> {
    let rows = runtime
        .read(CancellationToken::new(), |services| async move {
            services
                .executor
                .execute(
                    "SELECT source_root, imported_count, matched_count, conflict_count, error_count, discovered_count FROM migration_import_runs WHERE source_root LIKE 'meeting-import:%' ORDER BY started_at DESC LIMIT 20".into(),
                    vec![],
                )
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    Ok(rows
        .iter()
        .map(|row| ImportRun {
            provider_id: row["source_root"]
                .as_str()
                .unwrap_or_default()
                .trim_start_matches("meeting-import:")
                .to_owned(),
            result: ImportResult {
                discovered: row["discovered_count"].as_i64().unwrap_or(0),
                imported: row["imported_count"].as_i64().unwrap_or(0),
                matched: row["matched_count"].as_i64().unwrap_or(0),
                conflicts: row["conflict_count"].as_i64().unwrap_or(0),
                errors: row["error_count"].as_i64().unwrap_or(0),
            },
        })
        .collect())
}

/// `getImportedMeetingIds`: external IDs already imported from the provider.
pub async fn known_ids(runtime: &RuntimeHandle, provider: &str) -> Result<Vec<String>> {
    let provider = provider.to_owned();
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(
                    "SELECT external_event_id FROM sessions WHERE external_provider = ? AND external_event_id <> '' AND deleted_at IS NULL".into(),
                    vec![json!(provider)],
                )
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    Ok(rows
        .iter()
        .filter_map(|row| row["external_event_id"].as_str().map(str::to_owned))
        .collect())
}

/// Reads export files with the same limits as the generic file import.
pub fn read_files(paths: Vec<PathBuf>) -> Result<Vec<TextFile>> {
    if paths.is_empty() || paths.len() > 1000 {
        return Err(failure("Select between 1 and 1,000 files"));
    }
    let mut total = 0;
    paths
        .into_iter()
        .map(|path| {
            let metadata = std::fs::metadata(&path).map_err(failure)?;
            total += metadata.len();
            if !metadata.is_file() || metadata.len() > 20 * 1024 * 1024 || total > 100 * 1024 * 1024
            {
                return Err(failure("Import exceeds 20 MB per file or 100 MB total"));
            }
            let mut content = String::new();
            std::fs::File::open(&path)
                .map_err(failure)?
                .take(20 * 1024 * 1024)
                .read_to_string(&mut content)
                .map_err(failure)?;
            Ok(TextFile {
                path: path.display().to_string(),
                content,
            })
        })
        .collect()
}

/// `runMeetingImport`: each file once per content hash, each meeting once per stable ID.
/// Meetings that already exist count as unchanged for connections and as needing review for
/// export files.
pub async fn run(
    runtime: &RuntimeHandle,
    provider: &str,
    files: Vec<TextFile>,
    mode: Mode,
) -> Result<ImportResult> {
    let mut totals = ImportResult::default();
    if files.is_empty() {
        return match mode {
            Mode::Connection => Ok(totals),
            Mode::Export => Err(failure("Select at least one export file")),
        };
    }
    let run_id = uuid::Uuid::new_v4().to_string();
    let source_kind = format!(
        "meeting-{}:{provider}",
        match mode {
            Mode::Connection => "connection",
            Mode::Export => "export",
        }
    );
    transaction(
        runtime,
        vec![data::statement(
            "INSERT INTO migration_import_runs (id, importer_version, source_root, dry_run, status) VALUES (?, ?, ?, 0, 'running')",
            vec![json!(run_id), json!(IMPORTER_VERSION), json!(format!("meeting-import:{provider}"))],
        )],
    )
    .await?;
    for (index, file) in files.into_iter().enumerate() {
        let item = format!("{run_id}:item:{index}");
        let sha = sha256(&file.content);
        if let Some(prior) = prior_import(runtime, &file.path, &source_kind, &sha).await? {
            totals.discovered += prior;
            totals.matched += prior;
            transaction(runtime, vec![data::statement(
                "INSERT INTO migration_import_items (id, run_id, source_path, source_kind, source_sha256, status, discovered_count, imported_count, matched_count, completed_at) VALUES (?, ?, ?, ?, ?, 'unchanged', ?, 0, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                vec![json!(item), json!(run_id), json!(file.path), json!(source_kind), json!(sha), json!(prior), json!(prior)],
            )]).await?;
            continue;
        }
        match import_file(
            runtime,
            provider,
            &run_id,
            &item,
            &file,
            &sha,
            &source_kind,
            mode,
        )
        .await
        {
            Ok(result) => {
                totals.discovered += result.discovered;
                totals.imported += result.imported;
                totals.matched += result.matched;
                totals.conflicts += result.conflicts;
            }
            Err(error) => {
                totals.discovered += 1;
                totals.errors += 1;
                transaction(runtime, vec![data::statement(
                    "INSERT INTO migration_import_items (id, run_id, source_path, source_kind, source_sha256, status, discovered_count, skipped_count, error, completed_at) VALUES (?, ?, ?, ?, ?, 'error', 1, 1, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
                    vec![json!(item), json!(run_id), json!(file.path), json!(source_kind), json!(sha), json!(error.to_string())],
                )]).await?;
            }
        }
    }
    let status = if totals.errors > 0 {
        "completed_with_issues"
    } else if totals.conflicts > 0 {
        "completed_with_conflicts"
    } else {
        "completed"
    };
    transaction(runtime, vec![data::statement(
        "UPDATE migration_import_runs SET status = ?, discovered_count = ?, imported_count = ?, matched_count = ?, skipped_count = ?, conflict_count = ?, error_count = ?, completed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        vec![json!(status), json!(totals.discovered), json!(totals.imported), json!(totals.matched), json!(totals.errors), json!(totals.conflicts), json!(totals.errors), json!(run_id)],
    )]).await?;
    Ok(totals)
}

#[allow(clippy::too_many_arguments)]
async fn import_file(
    runtime: &RuntimeHandle,
    provider: &str,
    run_id: &str,
    item: &str,
    file: &TextFile,
    sha: &str,
    source_kind: &str,
    mode: Mode,
) -> Result<ImportResult> {
    let path = std::path::Path::new(&file.path);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_lowercase();
    let title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported meeting");
    let mut meetings = data::parse(&extension, title, &file.content)?;
    let external = external_ids(&extension, &file.content, meetings.len());
    let mut targets = Vec::new();
    for (index, meeting) in meetings.iter_mut().enumerate() {
        let external = external.get(index).cloned().flatten();
        let identity = external
            .clone()
            .unwrap_or_else(|| format!("{}#{index}", file.path));
        let id = format!(
            "meeting-import:{}",
            sha256(&format!("{provider}:{identity}"))
        );
        rebind(meeting, &id, provider, external.as_deref().unwrap_or(""));
        targets.push(id);
    }
    let existing = existing_ids(runtime, targets.clone()).await?;
    let new = meetings
        .into_iter()
        .zip(&targets)
        .filter(|(_, id)| !existing.contains(*id))
        .map(|(meeting, _)| meeting)
        .collect::<Vec<_>>();
    let imported = new.len() as i64;
    if !new.is_empty() {
        data::import_meetings(runtime, new, CancellationToken::new()).await?;
    }
    let existing_count = targets.len() as i64 - imported;
    let (matched, conflicts) = match mode {
        Mode::Connection => (existing_count, 0),
        Mode::Export => (0, existing_count),
    };
    let mut statements = vec![data::statement(
        "INSERT INTO migration_import_items (id, run_id, source_path, source_kind, source_sha256, status, discovered_count, imported_count, matched_count, skipped_count, conflict_count, error, completed_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 0, ?, '', strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        vec![
            json!(item),
            json!(run_id),
            json!(file.path),
            json!(source_kind),
            json!(sha),
            json!(if conflicts > 0 {
                "conflict"
            } else {
                "complete"
            }),
            json!(targets.len()),
            json!(imported),
            json!(matched),
            json!(conflicts),
        ],
    )];
    for id in &targets {
        let status = match (existing.contains(id), mode) {
            (false, _) => "inserted",
            (true, Mode::Connection) => "matched",
            (true, Mode::Export) => "conflict",
        };
        statements.push(data::statement(
            "INSERT INTO migration_import_targets (id, run_id, item_id, source_path, source_kind, table_name, target_id, status) VALUES (?, ?, ?, ?, ?, 'sessions', ?, ?)",
            vec![json!(format!("{item}:session:{id}")), json!(run_id), json!(item), json!(file.path), json!(source_kind), json!(id), json!(status)],
        ));
    }
    transaction(runtime, statements).await?;
    Ok(ImportResult {
        discovered: targets.len() as i64,
        imported,
        matched,
        conflicts,
        errors: 0,
    })
}

/// Gives a parsed meeting its stable session ID and provider identity.
fn rebind(meeting: &mut data::CanonicalMeeting, id: &str, provider: &str, external: &str) {
    meeting.session["id"] = json!(id);
    meeting.session["external_provider"] = json!(provider);
    meeting.session["external_event_id"] = json!(external);
    for row in meeting
        .documents
        .iter_mut()
        .chain(meeting.transcripts.iter_mut())
        .chain(meeting.participants.iter_mut())
    {
        row["session_id"] = json!(id);
    }
    if let Some(document) = meeting.documents.first_mut() {
        document["id"] = json!(id);
    }
}

/// Each JSON record's own identifier, in the order the generic parser reads records.
fn external_ids(extension: &str, content: &str, count: usize) -> Vec<Option<String>> {
    const KEYS: &[&str] = &[
        "id",
        "meeting_id",
        "meetingId",
        "transcript_id",
        "transcriptId",
        "document_id",
        "documentId",
        "call_id",
        "callId",
        "recording_id",
        "recordingId",
        "conversation_id",
        "conversationId",
        "uuid",
    ];
    let records = (extension == "json")
        .then(|| serde_json::from_str::<Value>(content).ok())
        .flatten()
        .map(|value| match value {
            Value::Array(items) => items,
            value => [
                "meetings",
                "conversations",
                "documents",
                "notes",
                "recordings",
                "results",
                "items",
                "data",
            ]
            .iter()
            .find_map(|key| value[*key].as_array().cloned())
            .unwrap_or_else(|| vec![value]),
        })
        .unwrap_or_default();
    if records.len() != count {
        return vec![None; count];
    }
    records
        .iter()
        .map(|record| {
            KEYS.iter().find_map(|key| match &record[*key] {
                Value::String(text) if !text.trim().is_empty() => Some(text.trim().to_owned()),
                Value::Number(number) => Some(number.to_string()),
                _ => None,
            })
        })
        .collect()
}

async fn prior_import(
    runtime: &RuntimeHandle,
    path: &str,
    kind: &str,
    sha: &str,
) -> Result<Option<i64>> {
    let params = vec![
        json!(path),
        json!(kind),
        json!(sha),
        json!(IMPORTER_VERSION),
    ];
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services.executor.execute(
                "SELECT item.discovered_count FROM migration_import_items AS item JOIN migration_import_runs AS run ON run.id = item.run_id WHERE item.source_path = ? AND item.source_kind = ? AND item.source_sha256 = ? AND item.status IN ('complete', 'unchanged', 'conflict') AND run.importer_version = ? AND run.dry_run = 0 ORDER BY item.created_at DESC LIMIT 1".into(),
                params,
            ).await.map_err(failure)
        })?
        .receive()
        .await?;
    Ok(rows
        .first()
        .and_then(|row| row["discovered_count"].as_i64()))
}

async fn existing_ids(
    runtime: &RuntimeHandle,
    ids: Vec<String>,
) -> Result<std::collections::HashSet<String>> {
    if ids.is_empty() {
        return Ok(Default::default());
    }
    let sql = format!(
        "SELECT id FROM sessions WHERE id IN ({})",
        vec!["?"; ids.len()].join(",")
    );
    let rows = runtime
        .read(CancellationToken::new(), move |services| async move {
            services
                .executor
                .execute(sql, ids.into_iter().map(Value::String).collect())
                .await
                .map_err(failure)
        })?
        .receive()
        .await?;
    Ok(rows
        .iter()
        .filter_map(|row| row["id"].as_str().map(str::to_owned))
        .collect())
}

async fn transaction(runtime: &RuntimeHandle, statements: Vec<TransactionStatement>) -> Result<()> {
    runtime
        .submit(move |services| async move {
            services
                .executor
                .execute_transaction(statements)
                .await
                .map_err(failure)?;
            Ok(())
        })?
        .receive()
        .await
}

fn sha256(value: &str) -> String {
    Sha256::digest(value.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use desktop_runtime::Profile;

    use super::*;

    #[test]
    fn detection_matches_names_and_bundle_ids_and_always_offers_google_meet() {
        let app = |id: &str, name: &str| InstalledApp {
            id: id.into(),
            name: name.into(),
        };
        let found = detect(&[
            app("ai.krisp.krispMac", "Krisp Desktop"),
            app("x", "Otter.app"),
        ])
        .iter()
        .map(|provider| provider.id)
        .collect::<Vec<_>>();
        assert_eq!(found, ["google-meet", "krisp", "otter"]);
    }

    #[test]
    fn repeated_imports_keep_one_session_per_meeting() {
        let directory = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(async {
            ready.receive().await.unwrap();
            let file = |content: &str| TextFile {
                path: "oauth://fathom/m1.json".into(),
                content: content.into(),
            };
            let first = run(
                &runtime,
                "fathom",
                vec![file(r#"{"id":"m1","title":"Sync","notes":"a"}"#)],
                Mode::Connection,
            )
            .await
            .unwrap();
            assert_eq!((first.imported, first.matched), (1, 0));
            // The same file is recognized by its content; an edited one by its meeting ID.
            let same = run(
                &runtime,
                "fathom",
                vec![file(r#"{"id":"m1","title":"Sync","notes":"a"}"#)],
                Mode::Connection,
            )
            .await
            .unwrap();
            let edited = run(
                &runtime,
                "fathom",
                vec![file(r#"{"id":"m1","title":"Sync","notes":"b"}"#)],
                Mode::Export,
            )
            .await
            .unwrap();
            assert_eq!((same.imported, same.matched), (0, 1));
            assert_eq!((edited.imported, edited.conflicts), (0, 1));
            assert_eq!(known_ids(&runtime, "fathom").await.unwrap(), ["m1"]);
            assert_eq!(history(&runtime).await.unwrap().len(), 3);
            runtime.shutdown().await.unwrap();
        });
    }
}
