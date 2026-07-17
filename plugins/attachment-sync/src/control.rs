use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;
use uuid::{Uuid, Version};

use crate::error::{Error, Result};

#[derive(Clone, Default)]
pub(crate) struct DownloadControl {
    inner: Arc<Mutex<DownloadControlInner>>,
}

#[derive(Default)]
struct DownloadControlInner {
    operations: HashMap<String, DownloadEntry>,
    clearing_scopes: HashSet<String>,
}

struct DownloadEntry {
    scope_id: Option<String>,
    cancellation: CancellationToken,
    finished: CancellationToken,
    state: DownloadState,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DownloadState {
    Registered,
    Running,
    Committing,
}

pub(crate) struct DownloadOperation {
    operation_id: String,
    cancellation: CancellationToken,
    control: DownloadControl,
}

pub(crate) struct SharedScopeClear {
    scope_id: String,
    pending: Vec<CancellationToken>,
    control: DownloadControl,
}

impl DownloadControl {
    pub(crate) fn begin(&self, operation_id: &str, scope_id: Option<&str>) -> Result<()> {
        validate_operation_id(operation_id)?;
        if let Some(scope_id) = scope_id {
            validate_scope_id(scope_id)?;
        }

        let mut inner = self.inner.lock().map_err(|_| Error::CacheUnavailable)?;
        if inner.operations.contains_key(operation_id)
            || scope_id.is_some_and(|scope_id| inner.clearing_scopes.contains(scope_id))
        {
            return Err(Error::InvalidTransferState);
        }
        inner.operations.insert(
            operation_id.to_string(),
            DownloadEntry {
                scope_id: scope_id.map(str::to_string),
                cancellation: CancellationToken::new(),
                finished: CancellationToken::new(),
                state: DownloadState::Registered,
            },
        );
        Ok(())
    }

    pub(crate) fn start(
        &self,
        operation_id: &str,
        scope_id: Option<&str>,
    ) -> Result<DownloadOperation> {
        let mut inner = self.inner.lock().map_err(|_| Error::CacheUnavailable)?;
        if scope_id.is_some_and(|scope_id| inner.clearing_scopes.contains(scope_id)) {
            return Err(Error::InvalidTransferState);
        }
        let entry = inner
            .operations
            .get_mut(operation_id)
            .ok_or(Error::InvalidTransferState)?;
        if entry.state != DownloadState::Registered || entry.scope_id.as_deref() != scope_id {
            return Err(Error::InvalidTransferState);
        }
        if entry.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        entry.state = DownloadState::Running;
        Ok(DownloadOperation {
            operation_id: operation_id.to_string(),
            cancellation: entry.cancellation.clone(),
            control: self.clone(),
        })
    }

    pub(crate) fn cancel(&self, operation_id: &str) -> Result<bool> {
        validate_operation_id(operation_id)?;
        let mut inner = self.inner.lock().map_err(|_| Error::CacheUnavailable)?;
        let Some(entry) = inner.operations.get(operation_id) else {
            return Ok(false);
        };
        entry.cancellation.cancel();
        if entry.state == DownloadState::Registered {
            let entry = inner
                .operations
                .remove(operation_id)
                .ok_or(Error::InvalidTransferState)?;
            entry.finished.cancel();
        }
        Ok(true)
    }

    pub(crate) fn begin_scope_clear(&self, scope_id: &str) -> Result<SharedScopeClear> {
        validate_scope_id(scope_id)?;
        let mut inner = self.inner.lock().map_err(|_| Error::CacheUnavailable)?;
        if !inner.clearing_scopes.insert(scope_id.to_string()) {
            return Err(Error::InvalidTransferState);
        }

        let operation_ids = inner
            .operations
            .iter()
            .filter(|(_, entry)| entry.scope_id.as_deref() == Some(scope_id))
            .map(|(operation_id, _)| operation_id.clone())
            .collect::<Vec<_>>();
        let mut pending = Vec::new();
        for operation_id in operation_ids {
            let Some(entry) = inner.operations.get(&operation_id) else {
                continue;
            };
            entry.cancellation.cancel();
            if entry.state == DownloadState::Registered {
                if let Some(entry) = inner.operations.remove(&operation_id) {
                    entry.finished.cancel();
                }
            } else {
                pending.push(entry.finished.clone());
            }
        }

        Ok(SharedScopeClear {
            scope_id: scope_id.to_string(),
            pending,
            control: self.clone(),
        })
    }

    fn begin_commit(&self, operation_id: &str) -> Result<()> {
        let mut inner = self.inner.lock().map_err(|_| Error::CacheUnavailable)?;
        let entry = inner
            .operations
            .get_mut(operation_id)
            .ok_or(Error::InvalidTransferState)?;
        if entry.state != DownloadState::Running {
            return Err(Error::InvalidTransferState);
        }
        if entry.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        entry.state = DownloadState::Committing;
        Ok(())
    }

    fn finish(&self, operation_id: &str) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        if let Some(entry) = inner.operations.remove(operation_id) {
            entry.finished.cancel();
        }
    }

    fn finish_scope_clear(&self, scope_id: &str) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.clearing_scopes.remove(scope_id);
        }
    }
}

impl DownloadOperation {
    pub(crate) fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    pub(crate) fn ensure_active(&self) -> Result<()> {
        if self.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(())
    }

    pub(crate) fn begin_commit(&self) -> Result<()> {
        self.control.begin_commit(&self.operation_id)
    }
}

impl Drop for DownloadOperation {
    fn drop(&mut self) {
        self.control.finish(&self.operation_id);
    }
}

impl SharedScopeClear {
    pub(crate) async fn wait(&self) {
        for finished in &self.pending {
            finished.cancelled().await;
        }
    }
}

impl Drop for SharedScopeClear {
    fn drop(&mut self) {
        self.control.finish_scope_clear(&self.scope_id);
    }
}

fn validate_operation_id(value: &str) -> Result<()> {
    let uuid = Uuid::parse_str(value).map_err(|_| Error::InvalidTransferState)?;
    if uuid.to_string() != value || uuid.get_version() != Some(Version::Random) {
        return Err(Error::InvalidTransferState);
    }
    Ok(())
}

fn validate_scope_id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 512
        || value
            .chars()
            .any(|character| character == '\0' || character.is_control())
    {
        return Err(Error::InvalidTransferState);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operation_id() -> String {
        Uuid::new_v4().to_string()
    }

    #[test]
    fn cancellation_wins_before_commit() {
        let control = DownloadControl::default();
        let operation_id = operation_id();
        control.begin(&operation_id, None).unwrap();
        let operation = control.start(&operation_id, None).unwrap();

        assert!(control.cancel(&operation_id).unwrap());
        assert!(matches!(operation.begin_commit(), Err(Error::Cancelled)));
    }

    #[test]
    fn commit_is_a_linearization_boundary() {
        let control = DownloadControl::default();
        let operation_id = operation_id();
        control.begin(&operation_id, None).unwrap();
        let operation = control.start(&operation_id, None).unwrap();

        operation.begin_commit().unwrap();
        assert!(control.cancel(&operation_id).unwrap());
        assert!(operation.ensure_active().is_err());
    }

    #[tokio::test]
    async fn scope_clear_cancels_and_drains_prior_downloads() {
        let control = DownloadControl::default();
        let first_operation_id = operation_id();
        control
            .begin(&first_operation_id, Some("viewer-a"))
            .unwrap();
        let operation = control
            .start(&first_operation_id, Some("viewer-a"))
            .unwrap();

        let clear = control.begin_scope_clear("viewer-a").unwrap();
        assert!(operation.ensure_active().is_err());
        assert!(control.begin(&operation_id(), Some("viewer-a")).is_err());

        drop(operation);
        clear.wait().await;
        drop(clear);
        control.begin(&operation_id(), Some("viewer-a")).unwrap();
    }

    #[tokio::test]
    async fn scope_clear_removes_downloads_that_have_not_started() {
        let control = DownloadControl::default();
        let operation_id = operation_id();
        control.begin(&operation_id, Some("viewer-a")).unwrap();

        let clear = control.begin_scope_clear("viewer-a").unwrap();
        clear.wait().await;
        assert!(control.start(&operation_id, Some("viewer-a")).is_err());
    }
}
