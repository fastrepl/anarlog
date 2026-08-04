use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use notify::RecursiveMode;
use notify_debouncer_full::{DebouncedEvent, new_debouncer};
use tauri_plugin_settings::SettingsPluginExt;
use tauri_specta::Event;

use crate::path::{should_skip_path, to_relative_path};
use crate::{FileChanged, WatcherState};

const DEBOUNCE_DELAY_MS: u64 = 900;
const OWN_WRITES_TTL_MS: u128 = (DEBOUNCE_DELAY_MS as u128) * 2 + 200;
pub const MAX_OWN_WRITES_PER_BATCH: usize = 4096;

fn compact_own_writes(own_writes: &mut HashMap<String, Instant>, now: Instant) {
    own_writes.retain(|_, timestamp| {
        now.saturating_duration_since(*timestamp).as_millis() < OWN_WRITES_TTL_MS
    });
}

#[derive(Debug, PartialEq)]
enum MarkOwnWritesResult {
    Marked,
    Wait(Duration),
    TooManyPaths,
}

fn try_mark_own_writes_at(
    own_writes: &mut HashMap<String, Instant>,
    paths: &[String],
    now: Instant,
) -> MarkOwnWritesResult {
    compact_own_writes(own_writes, now);

    if paths.len() > MAX_OWN_WRITES_PER_BATCH {
        return MarkOwnWritesResult::TooManyPaths;
    }
    let unique_paths: HashSet<_> = paths.iter().map(String::as_str).collect();

    let additional = unique_paths
        .iter()
        .filter(|path| !own_writes.contains_key(**path))
        .count();
    if own_writes.len() + additional <= MAX_OWN_WRITES_PER_BATCH {
        for path in unique_paths {
            own_writes.insert(path.to_string(), now);
        }
        return MarkOwnWritesResult::Marked;
    }

    let wait = own_writes
        .values()
        .map(|timestamp| {
            Duration::from_millis(OWN_WRITES_TTL_MS as u64)
                .saturating_sub(now.saturating_duration_since(*timestamp))
        })
        .min()
        .unwrap_or(Duration::from_millis(1))
        .max(Duration::from_millis(1));
    MarkOwnWritesResult::Wait(wait)
}

fn is_external_path(path: &str, own_writes: &mut HashMap<String, Instant>, now: Instant) -> bool {
    compact_own_writes(own_writes, now);
    !own_writes.contains_key(path)
}

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
                    let mut changed_paths = HashSet::new();

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

                    for path in changed_paths {
                        let should_emit = {
                            let mut own = own_writes.lock().unwrap();
                            is_external_path(&path, &mut own, Instant::now())
                        };
                        if !should_emit {
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
        let debouncer = state.debouncer.lock().unwrap().take();
        if let Some(debouncer) = debouncer {
            debouncer.stop();
        }
        state.own_writes.lock().unwrap().clear();
        Ok(())
    }

    pub fn mark_own_writes(&self, paths: &[String]) -> bool {
        let state = self.manager.state::<WatcherState>();
        loop {
            let result = {
                let mut guard = state.own_writes.lock().unwrap();
                try_mark_own_writes_at(&mut guard, paths, Instant::now())
            };
            match result {
                MarkOwnWritesResult::Marked => return true,
                MarkOwnWritesResult::Wait(duration) => std::thread::sleep(duration),
                MarkOwnWritesResult::TooManyPaths => {
                    tracing::error!(
                        count = paths.len(),
                        capacity = MAX_OWN_WRITES_PER_BATCH,
                        "own_write_batch_exceeds_capacity"
                    );
                    return false;
                }
            }
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

    #[test]
    fn detects_recent_own_writes_and_prunes_expired_entries() {
        let now = Instant::now();
        let mut own_writes = HashMap::from([
            (
                "recent.txt".to_string(),
                now - Duration::from_millis(OWN_WRITES_TTL_MS as u64 - 1),
            ),
            (
                "expired.txt".to_string(),
                now - Duration::from_millis(OWN_WRITES_TTL_MS as u64),
            ),
        ]);

        assert!(!is_external_path("recent.txt", &mut own_writes, now));
        assert_eq!(own_writes.len(), 1);
        assert!(own_writes.contains_key("recent.txt"));
        assert!(is_external_path("expired.txt", &mut own_writes, now));
        assert!(is_external_path("external.txt", &mut own_writes, now));
    }

    #[test]
    fn recognizes_unmarked_paths() {
        assert!(is_external_path(
            "external.txt",
            &mut HashMap::new(),
            Instant::now()
        ));
    }

    #[test]
    fn marking_own_writes_prunes_expired_entries_without_watcher_events() {
        let now = Instant::now();
        let mut own_writes = HashMap::from([(
            "expired.txt".to_string(),
            now - Duration::from_millis(OWN_WRITES_TTL_MS as u64),
        )]);

        assert_eq!(
            try_mark_own_writes_at(&mut own_writes, &["recent.txt".to_string()], now),
            MarkOwnWritesResult::Marked
        );

        assert_eq!(own_writes.len(), 1);
        assert!(own_writes.contains_key("recent.txt"));
    }

    #[test]
    fn full_own_write_registry_waits_without_evicting_live_entries() {
        let now = Instant::now();
        let paths: Vec<_> = (0..MAX_OWN_WRITES_PER_BATCH)
            .map(|index| format!("{index}.txt"))
            .collect();
        let mut own_writes = HashMap::new();
        assert_eq!(
            try_mark_own_writes_at(&mut own_writes, &paths, now),
            MarkOwnWritesResult::Marked
        );

        let result = try_mark_own_writes_at(
            &mut own_writes,
            &["next.txt".to_string()],
            now + Duration::from_millis(1),
        );

        assert!(matches!(result, MarkOwnWritesResult::Wait(_)));
        assert_eq!(own_writes.len(), MAX_OWN_WRITES_PER_BATCH);
        assert!(own_writes.contains_key("0.txt"));
        assert!(!own_writes.contains_key("next.txt"));
    }

    #[test]
    fn oversized_own_write_batch_is_rejected_without_mutation() {
        let now = Instant::now();
        let paths: Vec<_> = (0..=MAX_OWN_WRITES_PER_BATCH)
            .map(|index| format!("{index}.txt"))
            .collect();
        let mut own_writes = HashMap::from([("existing.txt".to_string(), now)]);

        assert_eq!(
            try_mark_own_writes_at(&mut own_writes, &paths, now),
            MarkOwnWritesResult::TooManyPaths
        );
        assert_eq!(own_writes.len(), 1);
        assert!(own_writes.contains_key("existing.txt"));
    }
}
