use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use tauri_plugin_settings::SettingsPluginExt;
use tauri_specta::Event;

use crate::path::{should_skip_path, to_relative_path};
use crate::{FileChanged, WatcherState};

const DEBOUNCE_DELAY_MS: u64 = 900;
const OWN_WRITES_TTL_MS: u128 = (DEBOUNCE_DELAY_MS as u128) * 2 + 200;

pub struct Notify<'a, R: tauri::Runtime, M: tauri::Manager<R>> {
    manager: &'a M,
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<'a, R: tauri::Runtime, M: tauri::Manager<R>> Notify<'a, R, M> {
    pub fn start(&self) -> Result<(), crate::Error> {
        let state = self.manager.state::<WatcherState>();
        let mut guard = state.debouncer.lock().unwrap();

        if guard.is_some() {
            return Ok(());
        }

        let base = self
            .manager
            .app_handle()
            .settings()
            .vault_base()?
            .into_std_path_buf();
        let app_handle = self.manager.app_handle().clone();
        let base_for_closure = base.clone();
        let own_writes = state.own_writes.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(DEBOUNCE_DELAY_MS),
            None,
            move |events: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
                if let Ok(events) = events {
                    let mut changed_paths: std::collections::HashSet<String> =
                        std::collections::HashSet::new();

                    for event in events {
                        let should_emit = match &event.kind {
                            notify::EventKind::Create(_) => true,
                            notify::EventKind::Remove(_) => true,

                            notify::EventKind::Any => false,
                            notify::EventKind::Access(_) | notify::EventKind::Other => false,
                            notify::EventKind::Modify(modify_kind) => {
                                matches!(
                                    modify_kind,
                                    notify::event::ModifyKind::Any
                                        | notify::event::ModifyKind::Data(_)
                                        | notify::event::ModifyKind::Name(_)
                                )
                            }
                        };

                        if !should_emit {
                            continue;
                        }

                        for path in &event.paths {
                            let relative_path = to_relative_path(path, &base_for_closure);

                            if should_skip_path(&relative_path, path) {
                                continue;
                            }

                            changed_paths.insert(relative_path);
                        }
                    }

                    {
                        let mut own = own_writes.lock().unwrap();
                        let now = std::time::Instant::now();
                        own.retain(|_, ts| now.duration_since(*ts).as_millis() < OWN_WRITES_TTL_MS);
                    }

                    for path in changed_paths {
                        let skip = {
                            let own = own_writes.lock().unwrap();
                            own.contains_key(&path)
                        };
                        if skip {
                            continue;
                        }
                        tracing::info!("file_changed: {:?}", path);
                        let _ = FileChanged { path }.emit(&app_handle);
                    }
                }
            },
        )?;

        debouncer.watch(&base, RecursiveMode::Recursive)?;
        *guard = Some(debouncer);

        Ok(())
    }

    pub fn stop(&self) -> Result<(), crate::Error> {
        let state = self.manager.state::<WatcherState>();
        let mut guard = state.debouncer.lock().unwrap();
        *guard = None;
        Ok(())
    }

    pub fn mark_own_writes(&self, paths: &[String]) {
        let state = self.manager.state::<WatcherState>();
        let mut guard = state.own_writes.lock().unwrap();
        let now = std::time::Instant::now();
        for path in paths {
            guard.insert(path.clone(), now);
        }
    }
}

pub trait NotifyPluginExt<R: tauri::Runtime> {
    fn notify(&self) -> Notify<'_, R, Self>
    where
        Self: tauri::Manager<R> + Sized;
}

impl<R: tauri::Runtime, T: tauri::Manager<R>> NotifyPluginExt<R> for T {
    fn notify(&self) -> Notify<'_, R, Self>
    where
        Self: Sized,
    {
        Notify {
            manager: self,
            _runtime: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    #[test]
    fn test_debounce_constants() {
        assert_eq!(DEBOUNCE_DELAY_MS, 900);
        assert_eq!(OWN_WRITES_TTL_MS, 2000);
    }

    #[test]
    fn test_own_writes_ttl_formula() {
        let expected = (DEBOUNCE_DELAY_MS as u128) * 2 + 200;
        assert_eq!(OWN_WRITES_TTL_MS, expected);
    }

    #[test]
    fn test_mark_own_writes_single_path() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        let paths = vec!["test/path.txt".to_string()];
        {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            for path in &paths {
                guard.insert(path.clone(), now);
            }
        }

        let guard = own_writes.lock().unwrap();
        assert!(guard.contains_key("test/path.txt"));
        assert_eq!(guard.len(), 1);
    }

    #[test]
    fn test_mark_own_writes_multiple_paths() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        let paths = vec![
            "path/one.txt".to_string(),
            "path/two.txt".to_string(),
            "path/three.txt".to_string(),
        ];
        {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            for path in &paths {
                guard.insert(path.clone(), now);
            }
        }

        let guard = own_writes.lock().unwrap();
        assert!(guard.contains_key("path/one.txt"));
        assert!(guard.contains_key("path/two.txt"));
        assert!(guard.contains_key("path/three.txt"));
        assert_eq!(guard.len(), 3);
    }

    #[test]
    fn test_mark_own_writes_updates_timestamp() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        let path = "test/path.txt".to_string();

        let first_ts = {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            guard.insert(path.clone(), now);
            now
        };

        std::thread::sleep(std::time::Duration::from_millis(10));

        let second_ts = {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            guard.insert(path.clone(), now);
            now
        };

        let guard = own_writes.lock().unwrap();
        let stored_ts = guard.get(&path).unwrap();
        assert!(*stored_ts > first_ts);
        assert_eq!(*stored_ts, second_ts);
    }

    #[test]
    fn test_own_writes_ttl_cleanup_retains_recent() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            guard.insert("recent.txt".to_string(), now);
        }

        {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            guard.retain(|_, ts| now.duration_since(*ts).as_millis() < OWN_WRITES_TTL_MS);
        }

        let guard = own_writes.lock().unwrap();
        assert!(guard.contains_key("recent.txt"));
    }

    #[test]
    fn test_own_writes_skip_logic() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        {
            let mut guard = own_writes.lock().unwrap();
            guard.insert("marked.txt".to_string(), Instant::now());
        }

        let changed_paths = vec!["marked.txt".to_string(), "not_marked.txt".to_string()];
        let mut emitted_paths = Vec::new();

        for path in changed_paths {
            let skip = {
                let own = own_writes.lock().unwrap();
                own.contains_key(&path)
            };
            if !skip {
                emitted_paths.push(path);
            }
        }

        assert_eq!(emitted_paths.len(), 1);
        assert_eq!(emitted_paths[0], "not_marked.txt");
    }

    #[test]
    fn test_own_writes_empty_paths() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        let paths: Vec<String> = vec![];
        {
            let mut guard = own_writes.lock().unwrap();
            let now = Instant::now();
            for path in &paths {
                guard.insert(path.clone(), now);
            }
        }

        let guard = own_writes.lock().unwrap();
        assert!(guard.is_empty());
    }

    #[test]
    fn test_own_writes_concurrent_access() {
        let own_writes: Arc<Mutex<HashMap<String, Instant>>> = Arc::new(Mutex::new(HashMap::new()));

        let own_writes_clone = own_writes.clone();
        let handle = std::thread::spawn(move || {
            let mut guard = own_writes_clone.lock().unwrap();
            guard.insert("thread1.txt".to_string(), Instant::now());
        });

        {
            let mut guard = own_writes.lock().unwrap();
            guard.insert("main.txt".to_string(), Instant::now());
        }

        handle.join().unwrap();

        let guard = own_writes.lock().unwrap();
        assert!(guard.contains_key("main.txt"));
        assert!(guard.contains_key("thread1.txt"));
    }
}
