use super::InstalledApp;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

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
    use libpulse_binding::context::{Context, FlagSet as ContextFlagSet};
    use libpulse_binding::mainloop::standard::{IterateResult, Mainloop};
    use std::cell::RefCell;
    use std::rc::Rc;

    let mut apps = Vec::new();

    let mut mainloop = Mainloop::new().ok_or(crate::Error::PulseMainloop)?;

    let mut context =
        Context::new(&mainloop, "hyprnote-detect").ok_or(crate::Error::PulseContext)?;

    context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .map_err(|_| crate::Error::PulseConnect)?;

    let apps_rc: Rc<RefCell<Vec<InstalledApp>>> = Rc::new(RefCell::new(Vec::new()));
    let apps_clone = apps_rc.clone();

    let introspect = context.introspect();
    introspect.get_source_output_info_list(move |result| {
        use libpulse_binding::callbacks::ListResult;

        if let ListResult::Item(info) = result {
            let props = &info.proplist;
            let app_name = props
                .get_str("application.name")
                .or_else(|| props.get_str("application.process.binary"))
                .unwrap_or_default();

            let app_id = props
                .get_str("application.process.binary")
                .or_else(|| props.get_str("application.name"))
                .unwrap_or_default();

            if !app_name.is_empty() && !app_id.is_empty() {
                apps_clone.borrow_mut().push(InstalledApp {
                    id: app_id,
                    name: app_name,
                });
            }
        }
    });

    for _ in 0..100 {
        match mainloop.iterate(false) {
            IterateResult::Quit(_) | IterateResult::Err(_) => break,
            IterateResult::Success(_) => {}
        }
    }

    context.disconnect();

    apps = apps_rc.borrow().clone();
    apps.sort_by(|a, b| a.id.cmp(&b.id));
    apps.dedup_by(|a, b| a.id == b.id);
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(apps)
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
