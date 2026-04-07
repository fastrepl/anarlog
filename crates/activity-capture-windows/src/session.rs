#![cfg(target_os = "windows")]

use std::sync::{Arc, Mutex};

use hypr_activity_capture_interface::{
    ActivityKind, AppIdKind, AppIdentity, CaptureCandidate, CaptureError, CapturePolicy,
    SnapshotSource,
};
use sysinfo::{Pid, ProcessesToUpdate, System};
use windows::{
    Win32::{
        Foundation::S_OK,
        Media::Audio::{
            AudioSessionStateActive, IAudioSessionControl, IAudioSessionControl2,
            IAudioSessionEnumerator, IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator,
            MMDeviceEnumerator, eConsole, eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree},
        UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
    },
    core::{Interface, PWSTR},
};

#[derive(Debug, Default)]
pub(crate) struct CaptureState {
    last_session_instance_id: Option<String>,
    last_pid: Option<u32>,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionCandidate {
    pub(crate) app: AppIdentity,
    pid: u32,
    session_instance_id: Option<String>,
    is_system_sounds: bool,
    matches_foreground: bool,
}

pub(crate) fn find_active_render_session(
    policy: &CapturePolicy,
    state: &Arc<Mutex<CaptureState>>,
) -> Result<Option<SessionCandidate>, CaptureError> {
    let candidates = list_active_render_sessions()?;
    Ok(select_preferred_session(state, policy, candidates))
}

pub(crate) fn clear_last_selected_session(state: &Arc<Mutex<CaptureState>>) {
    let mut state_guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state_guard.last_session_instance_id = None;
    state_guard.last_pid = None;
}

fn list_active_render_sessions() -> Result<Vec<SessionCandidate>, CaptureError> {
    let enumerator = match DeviceEnumerator::new() {
        Ok(enumerator) => enumerator,
        Err(_) => return Ok(Vec::new()),
    };
    let device = match enumerator.default_render_device() {
        Ok(device) => device,
        Err(_) => return Ok(Vec::new()),
    };
    let manager = match device.session_manager() {
        Ok(manager) => manager,
        Err(_) => return Ok(Vec::new()),
    };
    let sessions = match manager.session_enumerator() {
        Ok(sessions) => sessions,
        Err(_) => return Ok(Vec::new()),
    };
    let session_count = sessions.len()?;
    let foreground_pid = foreground_process_id();
    let mut system = System::new();

    let mut candidates = Vec::new();
    for index in 0..session_count {
        let session = match sessions.session(index) {
            Ok(session) => session,
            Err(_) => continue,
        };
        if !session.is_active() {
            continue;
        }

        candidates.push(build_session_candidate(
            session,
            foreground_pid,
            &mut system,
        ));
    }

    Ok(candidates)
}

fn build_session_candidate(
    session: AudioSession,
    foreground_pid: Option<u32>,
    system: &mut System,
) -> SessionCandidate {
    let pid = session.process_id().unwrap_or_default();
    let session_identifier = session.session_identifier();
    let session_instance_id = session.session_instance_identifier();
    let display_name = session.display_name();
    let is_system_sounds = session.is_system_sounds();
    let app = resolve_process_identity(
        system,
        pid,
        display_name.as_deref(),
        session_identifier.as_deref(),
        is_system_sounds,
    );

    SessionCandidate {
        app,
        pid,
        session_instance_id,
        is_system_sounds,
        matches_foreground: foreground_pid.is_some_and(|foreground_pid| foreground_pid == pid),
    }
}

fn foreground_process_id() -> Option<u32> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() {
        return None;
    }

    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }

    (pid != 0).then_some(pid)
}

fn select_preferred_session(
    state: &Arc<Mutex<CaptureState>>,
    policy: &CapturePolicy,
    candidates: Vec<SessionCandidate>,
) -> Option<SessionCandidate> {
    let mut regular_candidates = Vec::new();
    let mut system_sound_candidates = Vec::new();

    for candidate in candidates {
        let decision = policy.decision_for_candidate(&CaptureCandidate {
            app: candidate.app.clone(),
            activity_kind: ActivityKind::AudioSession,
            source: SnapshotSource::Workspace,
            browser: None,
        });
        if decision.skip || !decision.access.allows_snapshot() {
            continue;
        }

        if candidate.is_system_sounds {
            system_sound_candidates.push(candidate);
        } else {
            regular_candidates.push(candidate);
        }
    }

    let mut state_guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let selected = select_from_candidates(&state_guard, regular_candidates)
        .or_else(|| select_from_candidates(&state_guard, system_sound_candidates));

    if let Some(selected) = selected.as_ref() {
        state_guard.last_session_instance_id = selected.session_instance_id.clone();
        state_guard.last_pid = (selected.pid != 0).then_some(selected.pid);
    } else {
        state_guard.last_session_instance_id = None;
        state_guard.last_pid = None;
    }

    selected
}

fn select_from_candidates(
    state: &CaptureState,
    mut candidates: Vec<SessionCandidate>,
) -> Option<SessionCandidate> {
    candidates.sort_by(|left, right| compare_candidates(state, left, right));
    candidates.pop()
}

fn compare_candidates(
    state: &CaptureState,
    left: &SessionCandidate,
    right: &SessionCandidate,
) -> std::cmp::Ordering {
    candidate_priority_tuple(state, left)
        .cmp(&candidate_priority_tuple(state, right))
        .then_with(|| left.app.app_name.cmp(&right.app.app_name))
        .then_with(|| left.app.app_id.cmp(&right.app.app_id))
        .then_with(|| left.pid.cmp(&right.pid))
}

fn candidate_priority_tuple(
    state: &CaptureState,
    candidate: &SessionCandidate,
) -> (bool, bool, bool, bool, bool, bool, bool, u32) {
    let matches_previous_session = state.last_session_instance_id.as_ref().is_some_and(|id| {
        candidate
            .session_instance_id
            .as_ref()
            .is_some_and(|candidate_id| candidate_id == id)
    });
    let matches_previous_pid = state
        .last_pid
        .is_some_and(|last_pid| last_pid != 0 && last_pid == candidate.pid);

    (
        candidate.matches_foreground,
        matches_previous_session,
        matches_previous_pid,
        candidate.app.executable_path.is_some(),
        matches!(candidate.app.app_id_kind, AppIdKind::ExecutablePath),
        matches!(candidate.app.app_id_kind, AppIdKind::ProcessName),
        candidate.session_instance_id.is_some(),
        candidate.pid,
    )
}

fn resolve_process_identity(
    system: &mut System,
    pid: u32,
    display_name: Option<&str>,
    session_identifier: Option<&str>,
    is_system_sounds: bool,
) -> AppIdentity {
    if pid != 0 {
        let process_pid = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[process_pid]), true);

        if let Some(process) = system.process(process_pid) {
            let executable_path = process.exe().map(|path| path.to_string_lossy().to_string());
            let process_name = process.name().to_string_lossy().trim().to_string();
            let app_id_kind = if executable_path.is_some() {
                AppIdKind::ExecutablePath
            } else if !process_name.is_empty() {
                AppIdKind::ProcessName
            } else {
                AppIdKind::Pid
            };
            let app_id = executable_path
                .clone()
                .filter(|value| !value.is_empty())
                .or_else(|| (!process_name.is_empty()).then_some(process_name.clone()))
                .unwrap_or_else(|| format!("pid:{pid}"));

            return AppIdentity {
                pid: pid as i32,
                app_name: if process_name.is_empty() {
                    executable_path
                        .clone()
                        .unwrap_or_else(|| format!("pid:{pid}"))
                } else {
                    process_name
                },
                app_id,
                app_id_kind,
                bundle_id: None,
                executable_path,
            };
        }
    }

    let fallback_name = display_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            session_identifier
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            if is_system_sounds {
                "System Sounds".to_string()
            } else {
                format!("pid:{pid}")
            }
        });
    let (app_id, app_id_kind) = if pid != 0 {
        (format!("pid:{pid}"), AppIdKind::Pid)
    } else {
        (fallback_name.clone(), AppIdKind::ProcessName)
    };

    AppIdentity {
        pid: pid as i32,
        app_name: fallback_name,
        app_id,
        app_id_kind,
        bundle_id: None,
        executable_path: None,
    }
}

struct DeviceEnumerator {
    inner: IMMDeviceEnumerator,
}

impl DeviceEnumerator {
    fn new() -> Result<Self, CaptureError> {
        let inner = unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|error| CaptureError::platform(error.to_string()))?;
        Ok(Self { inner })
    }

    fn default_render_device(&self) -> Result<RenderDevice, CaptureError> {
        let inner = unsafe { self.inner.GetDefaultAudioEndpoint(eRender, eConsole) }
            .map_err(|error| CaptureError::platform(error.to_string()))?;
        Ok(RenderDevice { inner })
    }
}

struct RenderDevice {
    inner: IMMDevice,
}

impl RenderDevice {
    fn session_manager(&self) -> Result<AudioSessionManager, CaptureError> {
        let inner = unsafe {
            self.inner
                .Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)
        }
        .map_err(|error| CaptureError::platform(error.to_string()))?;
        Ok(AudioSessionManager { inner })
    }
}

struct AudioSessionManager {
    inner: IAudioSessionManager2,
}

impl AudioSessionManager {
    fn session_enumerator(&self) -> Result<AudioSessionEnumerator, CaptureError> {
        let inner = unsafe { self.inner.GetSessionEnumerator() }
            .map_err(|error| CaptureError::platform(error.to_string()))?;
        Ok(AudioSessionEnumerator { inner })
    }
}

struct AudioSessionEnumerator {
    inner: IAudioSessionEnumerator,
}

impl AudioSessionEnumerator {
    fn len(&self) -> Result<i32, CaptureError> {
        unsafe { self.inner.GetCount() }.map_err(|error| CaptureError::platform(error.to_string()))
    }

    fn session(&self, index: i32) -> Result<AudioSession, CaptureError> {
        let control = unsafe { self.inner.GetSession(index) }
            .map_err(|error| CaptureError::platform(error.to_string()))?;
        AudioSession::new(control)
    }
}

struct AudioSession {
    control: IAudioSessionControl,
    control2: IAudioSessionControl2,
}

impl AudioSession {
    fn new(control: IAudioSessionControl) -> Result<Self, CaptureError> {
        let control2 = control
            .cast()
            .map_err(|error| CaptureError::platform(error.to_string()))?;
        Ok(Self { control, control2 })
    }

    fn is_active(&self) -> bool {
        matches!(unsafe { self.control.GetState() }, Ok(state) if state == AudioSessionStateActive)
    }

    fn process_id(&self) -> Option<u32> {
        unsafe { self.control2.GetProcessId() }.ok()
    }

    fn session_identifier(&self) -> Option<String> {
        Self::co_task_mem_string(unsafe { self.control2.GetSessionIdentifier() })
    }

    fn session_instance_identifier(&self) -> Option<String> {
        Self::co_task_mem_string(unsafe { self.control2.GetSessionInstanceIdentifier() })
    }

    fn display_name(&self) -> Option<String> {
        Self::co_task_mem_string(unsafe { self.control.GetDisplayName() })
    }

    fn is_system_sounds(&self) -> bool {
        unsafe { self.control2.IsSystemSoundsSession() == S_OK }
    }

    fn co_task_mem_string(value: windows::core::Result<PWSTR>) -> Option<String> {
        CoTaskMemWideString::from_result(value).and_then(CoTaskMemWideString::into_string)
    }
}

// Session metadata APIs hand back CoTaskMem-allocated PWSTRs, and PWSTR itself does not own them.
struct CoTaskMemWideString(PWSTR);

impl CoTaskMemWideString {
    fn from_result(value: windows::core::Result<PWSTR>) -> Option<Self> {
        value.ok().and_then(Self::from_raw)
    }

    fn from_raw(value: PWSTR) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    fn into_string(self) -> Option<String> {
        let string = unsafe { self.0.to_string() }.ok()?;
        let string = string.trim();

        (!string.is_empty()).then(|| string.to_string())
    }
}

impl Drop for CoTaskMemWideString {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.0 as *const _));
        }
    }
}
