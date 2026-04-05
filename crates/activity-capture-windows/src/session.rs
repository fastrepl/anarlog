#![cfg(target_os = "windows")]

use std::{
    ffi::OsString,
    os::windows::ffi::OsStringExt,
    sync::{Arc, Mutex},
};

use hypr_activity_capture_interface::{
    ActivityKind, AppIdKind, AppIdentity, CaptureCandidate, CaptureError, CapturePolicy,
    SnapshotSource,
};
use sysinfo::{Pid, ProcessesToUpdate, System};
use windows::{
    Win32::{
        Media::Audio::{
            AudioSessionStateActive, IAudioSessionControl, IAudioSessionControl2,
            IAudioSessionManager2, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator, eConsole,
            eRender,
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
    let enumerator = match create_device_enumerator() {
        Ok(enumerator) => enumerator,
        Err(_) => return Ok(Vec::new()),
    };
    let device = match get_default_render_device(&enumerator) {
        Ok(device) => device,
        Err(_) => return Ok(Vec::new()),
    };
    let manager = match get_session_manager(&device) {
        Ok(manager) => manager,
        Err(_) => return Ok(Vec::new()),
    };
    let sessions = match unsafe { manager.GetSessionEnumerator() } {
        Ok(sessions) => sessions,
        Err(_) => return Ok(Vec::new()),
    };
    let session_count = unsafe { sessions.GetCount() }
        .map_err(|error| CaptureError::platform(error.to_string()))?;
    let foreground_pid = foreground_process_id();

    let mut candidates = Vec::new();
    for index in 0..session_count {
        let control = match unsafe { sessions.GetSession(index) } {
            Ok(control) => control,
            Err(_) => continue,
        };
        if !is_active_session(&control) {
            continue;
        }

        let control2: IAudioSessionControl2 = match control.cast() {
            Ok(control2) => control2,
            Err(_) => continue,
        };
        candidates.push(build_session_candidate(control, control2, foreground_pid));
    }

    Ok(candidates)
}

fn create_device_enumerator() -> Result<IMMDeviceEnumerator, CaptureError> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
        .map_err(|error| CaptureError::platform(error.to_string()))
}

fn get_default_render_device(enumerator: &IMMDeviceEnumerator) -> Result<IMMDevice, CaptureError> {
    unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole) }
        .map_err(|error| CaptureError::platform(error.to_string()))
}

fn get_session_manager(device: &IMMDevice) -> Result<IAudioSessionManager2, CaptureError> {
    unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) }
        .map_err(|error| CaptureError::platform(error.to_string()))
}

fn is_active_session(control: &IAudioSessionControl) -> bool {
    matches!(unsafe { control.GetState() }, Ok(state) if state == AudioSessionStateActive)
}

fn build_session_candidate(
    control: IAudioSessionControl,
    control2: IAudioSessionControl2,
    foreground_pid: Option<u32>,
) -> SessionCandidate {
    let pid = unsafe { control2.GetProcessId() }.unwrap_or_default();
    let session_identifier = pwstr_to_string(unsafe { control2.GetSessionIdentifier() }.ok());
    let session_instance_id =
        pwstr_to_string(unsafe { control2.GetSessionInstanceIdentifier() }.ok());
    let display_name = pwstr_to_string(unsafe { control.GetDisplayName() }.ok());
    let is_system_sounds = unsafe { control2.IsSystemSoundsSession().0 == 0 };
    let app = resolve_process_identity(
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

fn pwstr_to_string(value: Option<PWSTR>) -> Option<String> {
    let value = value?;
    if value.is_null() {
        return None;
    }

    let len = (0..)
        .take_while(|&index| unsafe { *value.0.add(index) != 0 })
        .count();
    let slice = unsafe { std::slice::from_raw_parts(value.0, len) };
    let string = OsString::from_wide(slice)
        .to_string_lossy()
        .trim()
        .to_string();

    unsafe {
        CoTaskMemFree(Some(value.0 as *const _));
    }

    (!string.is_empty()).then_some(string)
}

fn resolve_process_identity(
    pid: u32,
    display_name: Option<&str>,
    session_identifier: Option<&str>,
    is_system_sounds: bool,
) -> AppIdentity {
    if pid != 0 {
        let mut system = System::new();
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
