use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use anlg_listener_core::actors::recorder::delete_capture_audio;
use desktop_runtime::{Result, RuntimeHandle, ServiceError, SessionId};
use serde_json::json;

use super::model::{Retention, failure};
use super::store::{integer, string};

#[derive(Clone, Default)]
pub struct Activities(Arc<Mutex<ActivityState>>);

#[derive(Default)]
struct ActivityState {
    sessions: HashSet<SessionId>,
    frozen: bool,
}

pub struct StartupLease(Activities);

pub struct Activity {
    registry: Activities,
    session: SessionId,
}

impl Activities {
    pub fn acquire(&self, session: SessionId) -> Result<Activity> {
        let mut state = self.0.lock().map_err(failure)?;
        if state.frozen || !state.sessions.insert(session.clone()) {
            return Err(ServiceError::Busy);
        }
        Ok(Activity {
            registry: self.clone(),
            session,
        })
    }

    pub fn startup(&self) -> Result<StartupLease> {
        let mut state = self.0.lock().map_err(failure)?;
        if state.frozen || !state.sessions.is_empty() {
            return Err(ServiceError::Busy);
        }
        state.frozen = true;
        Ok(StartupLease(self.clone()))
    }
}

impl Drop for StartupLease {
    fn drop(&mut self) {
        self.0
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .frozen = false;
    }
}

impl Drop for Activity {
    fn drop(&mut self) {
        self.registry
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .sessions
            .remove(&self.session);
    }
}

pub fn session_directory(vault: &Path, session: &SessionId) -> Result<PathBuf> {
    let mut components = Path::new(session.0.as_ref()).components();
    if !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || session.0.contains('\\')
    {
        return Err(failure("Session ID is not a safe audio directory name."));
    }
    let sessions = vault.join("sessions");
    let directory = sessions.join(session.0.as_ref());
    for path in [&sessions, &directory] {
        match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(failure(
                    "Audio storage directory is a symlink; cleanup refused.",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(failure(error)),
        }
    }
    Ok(directory)
}

pub async fn cleanup(
    runtime: &RuntimeHandle,
    vault: PathBuf,
    activities: Activities,
    retention: Retention,
    now_ms: i64,
) -> Result<Vec<SessionId>> {
    let cutoff = match retention {
        Retention::Never => now_ms,
        Retention::Days(days) => now_ms.saturating_sub(i64::from(days) * 86_400_000),
        Retention::Forever => i64::MIN,
    };
    let mut cursor = String::new();
    let mut deleted = Vec::new();
    loop {
        let after = cursor.clone();
        let page = runtime.submit(move |services| async move {
            services.executor.execute(
                "SELECT id, CAST(unixepoch(created_at) * 1000 AS INTEGER) AS created_ms, EXISTS (SELECT 1 FROM session_attachments a LEFT JOIN attachment_local_state l ON l.attachment_id = a.id WHERE a.session_id = sessions.id AND a.source_type = 'session_audio' AND a.source_id = 'primary' AND a.deleted_at IS NOT NULL AND COALESCE(l.availability, 'present') != 'absent') AS tombstone FROM sessions WHERE id > ? AND NOT EXISTS (SELECT 1 FROM app_settings WHERE id = 'capture_lifecycle_pending:' || sessions.id) ORDER BY id LIMIT 100".into(),
                vec![json!(after)]).await.map_err(failure)
        })?.receive().await?;
        if page.is_empty() {
            break;
        }
        for row in page {
            cursor = string(&row, "id")?.to_owned();
            if integer(&row, "tombstone")? == 0
                && integer(&row, "created_ms").unwrap_or(i64::MAX) > cutoff
            {
                continue;
            }
            let session = SessionId(cursor.clone().into());
            let _lease = match activities.acquire(session.clone()) {
                Ok(lease) => lease,
                Err(ServiceError::Busy) => continue,
                Err(error) => return Err(error),
            };
            let directory = session_directory(&vault, &session)?;
            tokio::task::spawn_blocking(move || {
                if std::fs::symlink_metadata(&directory)
                    .is_ok_and(|metadata| metadata.file_type().is_symlink())
                {
                    return Err(failure("Refusing audio cleanup through a symlink."));
                }
                delete_capture_audio(&directory).map_err(failure)
            })
            .await
            .map_err(failure)??;
            let id = session.clone();
            runtime.submit(move |services| async move {
                services.executor.execute(
                    "INSERT INTO attachment_local_state (attachment_id, session_id, relative_path, availability) SELECT id, session_id, relative_path, 'absent' FROM session_attachments WHERE session_id = ? AND source_type = 'session_audio' AND source_id = 'primary' ON CONFLICT(attachment_id) DO UPDATE SET availability = 'absent', updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')".into(),
                    vec![json!(id)]).await.map_err(failure)?;
                Ok(())
            })?.receive().await?;
            deleted.push(session);
        }
    }
    Ok(deleted)
}
