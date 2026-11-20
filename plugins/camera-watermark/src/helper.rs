//! Anarlog Camera's two processes, from Anarlog's side (`tools/camera-watermark`). The login agent
//! (`anarlog-camera agent`) shows the person's camera whenever an app uses Anarlog Camera; it reads
//! what to show from `state.json` and reports in `status.json`. `anarlog-camera serve`, run here
//! for a moment at a time, adds the extension and registers the agent. Neither file nor process
//! is shared with Char Camera.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::prefs::CameraBackground;

/// The agent rewrites its status at least every 2 s; older than this, it isn't running.
pub const AGENT_STALE: Duration = Duration::from_secs(6);
/// How long `serve` stays up after its last request.
pub const BORROWED_SERVE: Duration = Duration::from_secs(20);

/// `AgentFiles.directory` in WatermarkKit.
pub fn agent_directory() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join("Library/Application Support/so.anarlog.camera"))
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum CameraAccess {
    Allowed,
    Denied,
    NotAsked,
}

/// The Camera Extension as macOS reported it after the last request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionState {
    Active,
    NeedsApproval,
    Failed,
    Removed,
}

/// The agent's launchd registration, as `SMAppService` reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AgentRegistration {
    Enabled,
    RequiresApproval,
    NotRegistered,
    NotFound,
}

/// The agent's `status` line.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AgentStatus {
    #[serde(default)]
    pub camera_installed: bool,
    #[serde(default)]
    pub consumers: Vec<String>,
    #[serde(default)]
    pub recording: bool,
    #[serde(default)]
    pub badge: bool,
    #[serde(default = "not_asked")]
    pub camera_permission: CameraAccess,
    #[serde(default)]
    pub source_camera: Option<String>,
    #[serde(default)]
    pub problem: Option<String>,
    /// What the agent asks each new frame to draw. Frames already on their way may still show the
    /// last one, so this alone never lets Anarlog remove a meeting app's effect.
    #[serde(default)]
    pub background: Option<String>,
    /// The request whose frames hide the room and went out lately (`ProtectedOutput`).
    #[serde(default)]
    pub protected_background_id: Option<String>,
}

fn not_asked() -> CameraAccess {
    CameraAccess::NotAsked
}

/// One line on `serve`'s stdout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelperEvent {
    Ready,
    Extension {
        state: ExtensionState,
        detail: Option<String>,
    },
    Agent {
        state: AgentRegistration,
        detail: Option<String>,
    },
    Camera {
        installed: bool,
    },
    Error {
        message: String,
    },
}

pub fn parse_event(line: &str) -> Option<HelperEvent> {
    #[derive(Deserialize)]
    #[serde(tag = "event", rename_all = "snake_case")]
    enum Wire {
        Ready,
        Extension {
            state: ExtensionState,
            detail: Option<String>,
        },
        Agent {
            state: AgentRegistration,
            detail: Option<String>,
        },
        Camera {
            #[serde(default)]
            installed: bool,
        },
        Error {
            message: Option<String>,
        },
    }
    Some(match serde_json::from_str::<Wire>(line).ok()? {
        Wire::Ready => HelperEvent::Ready,
        Wire::Extension { state, detail } => HelperEvent::Extension { state, detail },
        Wire::Agent { state, detail } => HelperEvent::Agent { state, detail },
        Wire::Camera { installed } => HelperEvent::Camera { installed },
        Wire::Error { message } => HelperEvent::Error {
            message: message.unwrap_or_else(|| "unknown".into()),
        },
    })
}

pub fn parse_status(line: &str) -> Option<AgentStatus> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if value.get("event")?.as_str()? != "status" {
        return None;
    }
    let mut status: AgentStatus = serde_json::from_value(value).ok()?;
    status.protected_background_id = status.protected_background_id.filter(|id| !id.is_empty());
    Some(status)
}

/// What Anarlog knows about Anarlog Camera right now.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The agent reported within `AGENT_STALE`.
    pub running: bool,
    pub status: Option<AgentStatus>,
    pub extension: Option<(ExtensionState, Option<String>)>,
    pub agent: Option<(AgentRegistration, Option<String>)>,
    /// Whether `serve` last found the device (`check`); None until it looked.
    pub camera_found: Option<bool>,
    /// The agent's camera access as it last reported it, kept between its reports.
    pub camera_permission: Option<CameraAccess>,
    /// The Background last written for the agent, and that request's id (`background_id`).
    pub background: CameraBackground,
    pub background_id: String,
    /// When the agent wrote the status above (ms since the epoch).
    pub status_written_at_ms: Option<u64>,
}

pub enum HelperCommand {
    Activate,
    EnableAgent,
    DisableAgent,
    Check,
}

impl HelperCommand {
    fn wire(&self) -> &'static str {
        match self {
            Self::Activate => r#"{"command":"activate"}"#,
            Self::EnableAgent => r#"{"command":"enable_agent"}"#,
            Self::DisableAgent => r#"{"command":"disable_agent"}"#,
            Self::Check => r#"{"command":"check"}"#,
        }
    }
}

struct Inner {
    enabled: bool,
    recording: bool,
    background: CameraBackground,
    background_id: String,
    camera_request: u64,
    serve: Option<(Child, ChildStdin)>,
    borrow: u64,
    extension: Option<(ExtensionState, Option<String>)>,
    agent: Option<(AgentRegistration, Option<String>)>,
    camera_found: Option<bool>,
    camera_permission: Option<CameraAccess>,
}

pub struct Helper {
    executable: Option<PathBuf>,
    directory: PathBuf,
    /// How long `serve` stays up after its last request.
    idle: Duration,
    inner: Mutex<Inner>,
    events: tokio::sync::broadcast::Sender<HelperEvent>,
}

impl Helper {
    /// `executable` is `anarlog-camera`, None when this build has no Anarlog Camera.
    pub fn new(executable: Option<PathBuf>, directory: PathBuf) -> Arc<Self> {
        Self::with_idle(executable, directory, BORROWED_SERVE)
    }

    fn with_idle(executable: Option<PathBuf>, directory: PathBuf, idle: Duration) -> Arc<Self> {
        Arc::new(Self {
            executable,
            directory,
            idle,
            inner: Mutex::new(Inner {
                enabled: false,
                recording: false,
                background: CameraBackground::None,
                background_id: uuid::Uuid::new_v4().to_string(),
                camera_request: 0,
                serve: None,
                borrow: 0,
                extension: None,
                agent: None,
                camera_found: None,
                camera_permission: None,
            }),
            events: tokio::sync::broadcast::channel(32).0,
        })
    }

    pub fn is_bundled(&self) -> bool {
        self.executable.as_deref().is_some_and(Path::exists)
    }

    /// The camera on: the agent runs and shows the camera whenever an app uses it.
    pub fn start(self: &Arc<Self>) {
        self.change(|inner| inner.enabled = true);
        self.send(HelperCommand::EnableAgent);
    }

    /// The camera off: the badge and the camera stop at once and the agent is removed. An app
    /// still showing Anarlog Camera gets the idle frame.
    pub fn stop(self: &Arc<Self>) {
        self.change(|inner| inner.enabled = false);
        self.send(HelperCommand::DisableAgent);
    }

    /// Anarlog is closing: the agent keeps showing the camera, without the badge.
    pub fn close(&self) {
        self.change(|inner| inner.recording = false);
        self.end_serve();
    }

    pub fn set_recording(&self, recording: bool) {
        let mut inner = self.lock();
        if std::mem::replace(&mut inner.recording, recording) != recording {
            self.write_state(&inner);
        }
    }

    /// Tells the agent what to put behind the person under a new request id, which only frames
    /// drawn for this request can answer with a receipt. Returns that id.
    pub fn set_background(&self, background: CameraBackground) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.change(|inner| {
            inner.background = background;
            inner.background_id = id.clone();
        });
        id
    }

    /// Has the agent ask macOS for camera access, which it reads the camera under.
    pub fn request_camera(&self) {
        self.change(|inner| inner.camera_request = now_ms());
    }

    fn change(&self, change: impl FnOnce(&mut Inner)) {
        let mut inner = self.lock();
        change(&mut inner);
        self.write_state(&inner);
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<HelperEvent> {
        self.events.subscribe()
    }

    pub fn snapshot(&self) -> Snapshot {
        let (status, written) = self.read_status();
        let mut inner = self.lock();
        if let Some(status) = &status {
            inner.camera_permission = Some(status.camera_permission);
        }
        Snapshot {
            running: status.is_some(),
            status,
            extension: inner.extension.clone(),
            agent: inner.agent.clone(),
            camera_found: inner.camera_found,
            camera_permission: inner.camera_permission,
            background: inner.background,
            background_id: inner.background_id.clone(),
            status_written_at_ms: written,
        }
    }

    /// Runs `serve` for a request; it stops again once it has been idle for `idle`. A `serve`
    /// that exited meanwhile is started again once.
    pub fn send(self: &Arc<Self>, command: HelperCommand) {
        let Some(executable) = self.executable.clone().filter(|path| path.exists()) else {
            return;
        };
        let borrow = {
            let mut inner = self.lock();
            for _ in 0..2 {
                if inner.serve.is_none() {
                    match self.spawn_serve(&executable) {
                        Ok(serve) => inner.serve = Some(serve),
                        Err(error) => {
                            tracing::warn!(%error, "camera_watermark_serve_failed");
                            return;
                        }
                    }
                }
                let written = inner
                    .serve
                    .as_mut()
                    .is_some_and(|(_, stdin)| writeln!(stdin, "{}", command.wire()).is_ok());
                if written {
                    break;
                }
                reap(inner.serve.take());
            }
            inner.borrow += 1;
            inner.borrow
        };
        let helper = Arc::downgrade(self);
        let idle = self.idle;
        std::thread::spawn(move || {
            std::thread::sleep(idle);
            let Some(helper) = helper.upgrade() else {
                return;
            };
            let unused = helper.lock().borrow == borrow;
            if unused {
                helper.end_serve();
            }
        });
    }

    fn spawn_serve(self: &Arc<Self>, executable: &Path) -> std::io::Result<(Child, ChildStdin)> {
        let mut child = Command::new(executable)
            .arg("serve")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdin = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let helper = Arc::downgrade(self);
        std::thread::spawn(move || {
            use std::io::BufRead;
            for line in std::io::BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let (Some(event), Some(helper)) = (parse_event(&line), helper.upgrade()) else {
                    continue;
                };
                helper.receive(event);
            }
        });
        Ok((child, stdin))
    }

    fn receive(&self, event: HelperEvent) {
        {
            let mut inner = self.lock();
            match &event {
                HelperEvent::Extension { state, detail } => {
                    inner.extension = Some((*state, detail.clone()))
                }
                HelperEvent::Agent { state, detail } => {
                    inner.agent = Some((*state, detail.clone()))
                }
                HelperEvent::Camera { installed } => inner.camera_found = Some(*installed),
                HelperEvent::Ready | HelperEvent::Error { .. } => {}
            }
        }
        let _ = self.events.send(event);
    }

    /// Closing stdin is `serve`'s signal to exit.
    fn end_serve(&self) {
        let serve = self.lock().serve.take();
        reap(serve);
    }

    /// Tells the agent what to show, written whole so it never reads half a file. Callers hold
    /// the lock until the file is in place: the observer, settings and the switch change this
    /// from different threads, and an older state must never land after a newer one or share
    /// the temporary file with it.
    fn write_state(&self, inner: &Inner) {
        let state = serde_json::json!({
            "enabled": inner.enabled,
            // Off, the camera never shows the badge.
            "recording": inner.enabled && inner.recording,
            "owner_pid": std::process::id(),
            "background": inner.background.wire(),
            "background_id": inner.background_id,
            "camera_request": inner.camera_request,
        });
        let path = self.directory.join("state.json");
        let temporary = self
            .directory
            .join(format!("state.json.{}.tmp", std::process::id()));
        let written = std::fs::create_dir_all(&self.directory)
            .and_then(|()| std::fs::write(&temporary, state.to_string()))
            .and_then(|()| std::fs::rename(&temporary, &path));
        if let Err(error) = written {
            tracing::warn!(%error, "camera_watermark_state_write_failed");
        }
    }

    /// The agent's latest status and when it wrote it, or None when it hasn't reported lately.
    fn read_status(&self) -> (Option<AgentStatus>, Option<u64>) {
        let path = self.directory.join("status.json");
        let Ok(modified) = std::fs::metadata(&path).and_then(|meta| meta.modified()) else {
            return (None, None);
        };
        let written = modified
            .duration_since(UNIX_EPOCH)
            .map(|elapsed| elapsed.as_millis() as u64)
            .unwrap_or(0);
        if now_ms().saturating_sub(written) >= AGENT_STALE.as_millis() as u64 {
            return (None, None);
        }
        match std::fs::read_to_string(&path)
            .ok()
            .as_deref()
            .and_then(parse_status)
        {
            Some(status) => (Some(status), Some(written)),
            None => (None, None),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Closes `serve`'s stdin, its signal to exit, and waits for it off this thread.
fn reap(serve: Option<(Child, ChildStdin)>) {
    if let Some((mut child, stdin)) = serve {
        drop(stdin);
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        self.end_serve();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(dir: &Path) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(dir.join("state.json")).unwrap()).unwrap()
    }

    fn report(dir: &Path, fields: serde_json::Value) {
        let mut line = serde_json::json!({ "event": "status", "camera_installed": true });
        line.as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        std::fs::write(dir.join("status.json"), line.to_string()).unwrap();
    }

    #[test]
    fn never_badges_a_camera_that_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let helper = Helper::new(None, dir.path().into());
        helper.set_recording(true);
        helper.request_camera();
        assert_eq!(state(dir.path())["recording"], false);
        assert_eq!(state(dir.path())["enabled"], false);
        helper.start();
        assert_eq!(state(dir.path())["recording"], true);
        assert_eq!(state(dir.path())["owner_pid"], std::process::id());
        helper.stop();
        assert_eq!(state(dir.path())["recording"], false);
    }

    #[test]
    fn closing_leaves_the_camera_on_without_the_badge() {
        let dir = tempfile::tempdir().unwrap();
        let helper = Helper::new(None, dir.path().into());
        helper.start();
        helper.set_recording(true);
        helper.close();
        assert_eq!(state(dir.path())["enabled"], true);
        assert_eq!(state(dir.path())["recording"], false);
    }

    #[test]
    fn names_each_background_request_so_a_receipt_never_vouches_for_the_next() {
        let dir = tempfile::tempdir().unwrap();
        let helper = Helper::new(None, dir.path().into());
        let first = helper.set_background(CameraBackground::Blur);
        assert_eq!(state(dir.path())["background_id"], first.as_str());
        assert_eq!(state(dir.path())["background"]["kind"], "blur");
        helper.set_background(CameraBackground::None);
        let second = helper.set_background(CameraBackground::Blur);
        assert_ne!(first, second);
        assert_eq!(helper.snapshot().background_id, second);
    }

    #[test]
    fn a_change_is_never_overwritten_by_an_older_state_from_another_thread() {
        const ROUNDS: usize = 200;
        let dir = tempfile::tempdir().unwrap();
        let helper = Helper::new(None, dir.path().into());
        helper.start();
        // The recording lifecycle moves the badge while the person changes their Background.
        let badge = {
            let (helper, dir) = (helper.clone(), dir.path().to_path_buf());
            std::thread::spawn(move || {
                for round in 0..ROUNDS {
                    let recording = round % 2 == 0;
                    helper.set_recording(recording);
                    assert_eq!(state(&dir)["recording"], recording, "badge round {round}");
                }
            })
        };
        for round in 0..ROUNDS {
            let background = if round % 2 == 0 {
                CameraBackground::Blur
            } else {
                CameraBackground::None
            };
            let id = helper.set_background(background);
            assert_eq!(
                state(dir.path())["background_id"],
                id.as_str(),
                "background round {round}"
            );
        }
        badge.join().unwrap();
    }

    #[test]
    fn reads_the_agents_status_while_it_reports() {
        let dir = tempfile::tempdir().unwrap();
        let helper = Helper::new(None, dir.path().into());
        assert!(!helper.snapshot().running);
        report(
            dir.path(),
            serde_json::json!({
                "consumers": ["us.zoom.xos"], "camera_permission": "allowed",
                "protected_background_id": ""
            }),
        );
        let snapshot = helper.snapshot();
        assert!(snapshot.running);
        let status = snapshot.status.unwrap();
        assert_eq!(status.consumers, vec!["us.zoom.xos".to_string()]);
        assert_eq!(status.camera_permission, CameraAccess::Allowed);
        assert_eq!(status.protected_background_id, None, "empty names nothing");
        assert!(snapshot.status_written_at_ms.is_some());
        assert_eq!(snapshot.camera_permission, Some(CameraAccess::Allowed));
    }

    #[test]
    fn a_status_too_old_means_the_agent_isnt_running() {
        let dir = tempfile::tempdir().unwrap();
        let helper = Helper::new(None, dir.path().into());
        report(
            dir.path(),
            serde_json::json!({ "camera_permission": "allowed" }),
        );
        let old = SystemTime::now() - AGENT_STALE - Duration::from_secs(1);
        std::fs::File::options()
            .write(true)
            .open(dir.path().join("status.json"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        let snapshot = helper.snapshot();
        assert!(!snapshot.running);
        assert_eq!(snapshot.status_written_at_ms, None);
    }

    #[test]
    fn parses_serves_events_and_ignores_anything_else() {
        assert_eq!(
            parse_event(r#"{"event":"ready"}"#),
            Some(HelperEvent::Ready)
        );
        assert_eq!(
            parse_event(r#"{"event":"extension","state":"failed","detail":"missing_entitlement"}"#),
            Some(HelperEvent::Extension {
                state: ExtensionState::Failed,
                detail: Some("missing_entitlement".into())
            })
        );
        assert_eq!(
            parse_event(r#"{"event":"agent","state":"requires_approval"}"#),
            Some(HelperEvent::Agent {
                state: AgentRegistration::RequiresApproval,
                detail: None
            })
        );
        assert_eq!(
            parse_event(r#"{"event":"camera","installed":true}"#),
            Some(HelperEvent::Camera { installed: true })
        );
        assert_eq!(
            parse_event(r#"{"event":"extension","state":"exploded"}"#),
            None
        );
        assert_eq!(parse_event("not json"), None);
        assert_eq!(parse_status(r#"{"event":"camera","installed":true}"#), None);
    }

    #[cfg(unix)]
    #[test]
    fn talks_to_serve_and_keeps_its_answers() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("commands.log");
        let file = dir.path().join("anarlog-camera");
        std::fs::write(
            &file,
            format!(
                "#!/bin/sh\necho '{{\"event\":\"ready\"}}'\nwhile read line; do\n  echo \"$line\" >> '{}'\n  case \"$line\" in\n    *enable_agent*) echo '{{\"event\":\"agent\",\"state\":\"enabled\"}}' ;;\n    *activate*) echo '{{\"event\":\"extension\",\"state\":\"needs_approval\"}}' ;;\n  esac\ndone\n",
                log.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        let helper = Helper::new(Some(file), dir.path().join("agent"));
        let mut events = helper.subscribe();
        helper.start();
        helper.send(HelperCommand::Activate);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let mut seen = Vec::new();
        while seen.len() < 3 && std::time::Instant::now() < deadline {
            match events.try_recv() {
                Ok(event) => seen.push(event),
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        assert!(seen.contains(&HelperEvent::Agent {
            state: AgentRegistration::Enabled,
            detail: None
        }));
        let snapshot = helper.snapshot();
        assert_eq!(
            snapshot.agent.map(|agent| agent.0),
            Some(AgentRegistration::Enabled)
        );
        assert_eq!(
            snapshot.extension.map(|extension| extension.0),
            Some(ExtensionState::NeedsApproval)
        );
        let commands = std::fs::read_to_string(&log).unwrap();
        assert_eq!(
            commands.lines().collect::<Vec<_>>(),
            vec![r#"{"command":"enable_agent"}"#, r#"{"command":"activate"}"#]
        );
    }

    #[cfg(unix)]
    #[test]
    fn serve_stops_once_idle_and_starts_again_for_the_next_request() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let starts = dir.path().join("starts.log");
        let file = dir.path().join("anarlog-camera");
        std::fs::write(
            &file,
            format!(
                "#!/bin/sh\necho start >> '{}'\nwhile read line; do :; done\n",
                starts.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();
        let helper = Helper::with_idle(
            Some(file),
            dir.path().join("agent"),
            Duration::from_millis(200),
        );
        helper.send(HelperCommand::Check);
        helper.send(HelperCommand::Check);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while helper.lock().serve.is_some() {
            assert!(std::time::Instant::now() < deadline, "serve never stopped");
            std::thread::sleep(Duration::from_millis(20));
        }
        helper.send(HelperCommand::Check);
        assert!(helper.lock().serve.is_some());
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::fs::read_to_string(&starts)
            .unwrap_or_default()
            .lines()
            .count()
            < 2
        {
            assert!(
                std::time::Instant::now() < deadline,
                "serve never restarted"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}
