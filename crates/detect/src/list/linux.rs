use super::InstalledApp;
use libpulse_binding as pulse;
use libpulse_binding::context::{Context, FlagSet as ContextFlagSet};
use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

const PULSE_OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
const PULSE_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub fn list_installed_apps() -> Vec<InstalledApp> {
    let desktop_dirs = get_desktop_file_dirs();
    let mut apps = HashMap::new();

    for dir in desktop_dirs {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("desktop")
                    && let Some(app) = parse_desktop_file(&path)
                {
                    apps.entry(app.id.clone()).or_insert(app);
                }
            }
        }
    }

    let mut result: Vec<InstalledApp> = apps.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

pub fn list_mic_using_apps() -> Result<Vec<InstalledApp>, crate::Error> {
    let mut mainloop = Mainloop::new().ok_or(crate::Error::PulseMainloop)?;
    let mut context =
        Context::new(&mainloop, "hyprnote-detect").ok_or(crate::Error::PulseContext)?;

    context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .map_err(|_| crate::Error::PulseConnect)?;

    if let Err(error) = wait_for_context_ready(&mut mainloop, &context) {
        context.disconnect();
        return Err(error);
    }

    let query = Rc::new(RefCell::new(SourceOutputQuery::default()));
    let query_for_callback = query.clone();
    let mut operation =
        context
            .introspect()
            .get_source_output_info_list(move |result| match result {
                pulse::callbacks::ListResult::Item(info) => {
                    if info.corked {
                        return;
                    }

                    let props = &info.proplist;
                    let name = props
                        .get_str("application.name")
                        .or_else(|| props.get_str("application.process.binary"))
                        .unwrap_or_default();
                    let id = props
                        .get_str("application.process.binary")
                        .or_else(|| props.get_str("application.name"))
                        .unwrap_or_default();

                    if !name.is_empty() && !id.is_empty() {
                        query_for_callback
                            .borrow_mut()
                            .apps
                            .push(InstalledApp { id, name });
                    }
                }
                pulse::callbacks::ListResult::End => {
                    query_for_callback.borrow_mut().completed = true;
                }
                pulse::callbacks::ListResult::Error => {
                    let mut query = query_for_callback.borrow_mut();
                    query.failed = true;
                    query.completed = true;
                }
            });

    let query_result = wait_for_source_output_query(&mut mainloop, &query);
    if query_result.is_err() && !query.borrow().completed {
        operation.cancel();
    }
    drop(operation);
    context.disconnect();
    query_result?;

    let mut apps = query.borrow().apps.clone();
    apps.sort_by(|a, b| a.id.cmp(&b.id));
    apps.dedup_by(|a, b| a.id == b.id);
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(apps)
}

#[derive(Default)]
struct SourceOutputQuery {
    apps: Vec<InstalledApp>,
    completed: bool,
    failed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextReadiness {
    Pending,
    Ready,
    Failed,
}

fn context_readiness(state: pulse::context::State) -> ContextReadiness {
    match state {
        pulse::context::State::Ready => ContextReadiness::Ready,
        pulse::context::State::Failed | pulse::context::State::Terminated => {
            ContextReadiness::Failed
        }
        _ => ContextReadiness::Pending,
    }
}

fn wait_for_context_ready(mainloop: &mut Mainloop, context: &Context) -> Result<(), crate::Error> {
    let deadline = Instant::now() + PULSE_OPERATION_TIMEOUT;

    loop {
        match context_readiness(context.get_state()) {
            ContextReadiness::Ready => return Ok(()),
            ContextReadiness::Failed => {
                return Err(crate::Error::PulseOperation(
                    "context failed before becoming ready".to_string(),
                ));
            }
            ContextReadiness::Pending => {}
        }

        if Instant::now() >= deadline {
            return Err(crate::Error::PulseOperation(
                "timed out waiting for context readiness".to_string(),
            ));
        }

        iterate_mainloop(mainloop, "waiting for context readiness")?;
    }
}

fn wait_for_source_output_query(
    mainloop: &mut Mainloop,
    query: &Rc<RefCell<SourceOutputQuery>>,
) -> Result<(), crate::Error> {
    let deadline = Instant::now() + PULSE_OPERATION_TIMEOUT;

    loop {
        let query = query.borrow();
        if query.completed {
            return if query.failed {
                Err(crate::Error::PulseOperation(
                    "source-output introspection failed".to_string(),
                ))
            } else {
                Ok(())
            };
        }
        drop(query);

        if Instant::now() >= deadline {
            return Err(crate::Error::PulseOperation(
                "timed out waiting for source-output introspection".to_string(),
            ));
        }

        iterate_mainloop(mainloop, "enumerating source outputs")?;
    }
}

fn iterate_mainloop(mainloop: &mut Mainloop, operation: &str) -> Result<(), crate::Error> {
    match mainloop.iterate(false) {
        IterateResult::Success(0) => std::thread::sleep(PULSE_POLL_INTERVAL),
        IterateResult::Success(_) => {}
        IterateResult::Quit(retval) => {
            return Err(crate::Error::PulseOperation(format!(
                "mainloop quit while {operation}: {retval:?}"
            )));
        }
        IterateResult::Err(error) => {
            return Err(crate::Error::PulseOperation(format!(
                "mainloop failed while {operation}: {error:?}"
            )));
        }
    }

    Ok(())
}

fn get_desktop_file_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs.push(PathBuf::from("/usr/local/share/applications"));

    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(format!("{}/.local/share/applications", home)));
    }

    if let Ok(xdg_data_dirs) = std::env::var("XDG_DATA_DIRS") {
        for dir in xdg_data_dirs.split(':') {
            if !dir.is_empty() {
                dirs.push(PathBuf::from(format!("{}/applications", dir)));
            }
        }
    }

    if let Ok(xdg_data_home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(PathBuf::from(format!("{}/applications", xdg_data_home)));
    }

    dirs
}

fn parse_desktop_file(path: &std::path::Path) -> Option<InstalledApp> {
    let content = fs::read_to_string(path).ok()?;
    let mut name = None;
    let mut id = None;
    let mut in_desktop_entry = false;

    for line in content.lines() {
        let line = line.trim();

        if line == "[Desktop Entry]" {
            in_desktop_entry = true;
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = false;
            continue;
        }

        if !in_desktop_entry {
            continue;
        }

        if line.starts_with("Name=") && name.is_none() {
            name = Some(line.strip_prefix("Name=")?.to_string());
        }

        if line.starts_with("Icon=") && id.is_none() {
            id = Some(line.strip_prefix("Icon=")?.to_string());
        }

        if line.starts_with("Exec=")
            && id.is_none()
            && let Some(exec) = line.strip_prefix("Exec=")
        {
            let binary = exec
                .split_whitespace()
                .next()?
                .split('/')
                .next_back()?
                .to_string();
            id = Some(binary);
        }
    }

    if id.is_none() {
        id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());
    }

    Some(InstalledApp {
        id: id?,
        name: name?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_readiness_requires_ready_state() {
        use pulse::context::State;

        for state in [
            State::Unconnected,
            State::Connecting,
            State::Authorizing,
            State::SettingName,
        ] {
            assert_eq!(context_readiness(state), ContextReadiness::Pending);
        }
        assert_eq!(context_readiness(State::Ready), ContextReadiness::Ready);
        assert_eq!(context_readiness(State::Failed), ContextReadiness::Failed);
        assert_eq!(
            context_readiness(State::Terminated),
            ContextReadiness::Failed
        );
    }

    #[test]
    #[ignore]
    fn test_list_installed_apps() {
        let apps = list_installed_apps();
        println!("Got {} apps\n---", apps.len());
        println!(
            "{}",
            apps.iter()
                .map(|a| format!("- {} ({})", a.name, a.id))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    #[ignore]
    fn test_list_mic_using_apps() {
        let apps = list_mic_using_apps().unwrap();
        println!("Got {} apps\n---", apps.len());
        println!(
            "{}",
            apps.iter()
                .map(|a| format!("- {} ({})", a.name, a.id))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
