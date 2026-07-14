use std::sync::Arc;

use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::broadcast;

use sqlx::sqlite::SqliteOperation;

use crate::tracker::{ChangeTracker, HookState};
use crate::{TableChange, TableChangeKind};

#[derive(Clone, Debug)]
pub struct ChangeNotifier {
    table_change_tx: broadcast::Sender<TableChange>,
    change_tracker: Arc<ChangeTracker>,
}

impl ChangeNotifier {
    pub fn new() -> (Self, SqlitePoolOptions) {
        Self::build(Some(false))
    }

    pub fn new_with_cloudsync() -> (Self, SqlitePoolOptions) {
        Self::build(Some(true))
    }

    pub fn disabled() -> (Self, SqlitePoolOptions) {
        Self::build(None)
    }

    fn build(cloudsync_enabled: Option<bool>) -> (Self, SqlitePoolOptions) {
        let (table_change_tx, _) = broadcast::channel(256);
        let change_tracker = Arc::new(ChangeTracker::default());

        let notifier = Self {
            table_change_tx,
            change_tracker,
        };

        let Some(cloudsync_enabled) = cloudsync_enabled else {
            return (notifier, SqlitePoolOptions::new());
        };

        let callback_tx = notifier.table_change_tx.clone();
        let callback_tracker = Arc::clone(&notifier.change_tracker);

        let pool_options = SqlitePoolOptions::new().after_connect(move |conn, _| {
            let callback_tx = callback_tx.clone();
            let callback_tracker = Arc::clone(&callback_tracker);

            Box::pin(async move {
                let mut handle = conn.lock_handle().await?;
                let hook_state = Arc::new(HookState::new(callback_tx, callback_tracker));

                let update_state = Arc::clone(&hook_state);
                handle.set_update_hook(move |update| {
                    if cloudsync_enabled && update.database != "main" {
                        return;
                    }
                    let kind = match update.operation {
                        SqliteOperation::Insert => TableChangeKind::Insert,
                        SqliteOperation::Update => TableChangeKind::Update,
                        SqliteOperation::Delete => TableChangeKind::Delete,
                        SqliteOperation::Unknown(_) => return,
                    };
                    update_state.record(update.table, kind);
                });

                let commit_state = Arc::clone(&hook_state);
                if cloudsync_enabled {
                    hypr_cloudsync::install_transaction_observer(
                        &mut handle,
                        move || commit_state.flush(),
                        move || hook_state.clear(),
                    )
                    .map_err(|error| sqlx::Error::Configuration(Box::new(error)))?;
                } else {
                    handle.set_commit_hook(move || {
                        commit_state.flush();
                        true
                    });

                    handle.set_rollback_hook(move || {
                        hook_state.clear();
                    });
                }

                Ok(())
            })
        });

        (notifier, pool_options)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<TableChange> {
        self.table_change_tx.subscribe()
    }

    pub fn current_seq(&self) -> u64 {
        self.change_tracker.current_seq()
    }

    pub fn latest_table_seq(&self, table: &str) -> Option<u64> {
        self.change_tracker.latest_table_seq(table)
    }
}
