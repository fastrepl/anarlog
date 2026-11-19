//! Installs the Developers page's services and keeps the Cloud API's server-readable copy current,
//! like shipping's `CloudApiBackfillLifecycle`: finish pending work at sign-in, then delete or
//! upload each changed meeting (uploads 1.5 seconds after the last change).

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::Context;
use serde_json::Value;

use super::ApplicationView;
use crate::{
    application_services::NativeServices,
    product::{
        cloud::{
            auth::SecureAuthProvider,
            cloud_api::{CloudApi, REVISIONS_SQL, Revision},
        },
        local::developers_view::Developers,
    },
};

/// Reads the tables `REVISIONS_SQL` reads, so it notifies on every change to them. The
/// revisions themselves are read in full: a library outgrows one watch's 1,000 rows.
const CHANGES_SQL: &str = "SELECT EXISTS(SELECT 1 FROM sessions) AS sessions,
    EXISTS(SELECT 1 FROM session_documents) AS documents,
    EXISTS(SELECT 1 FROM transcripts) AS transcripts,
    EXISTS(SELECT 1 FROM session_participants) AS participants,
    EXISTS(SELECT 1 FROM humans) AS humans,
    EXISTS(SELECT 1 FROM organizations) AS organizations,
    EXISTS(SELECT 1 FROM action_items) AS action_items";
const UPLOAD_DELAY: Duration = Duration::from_millis(1500);
/// Shipping retries a snapshot the API rejected as invalid after these delays.
const INVALID_RETRIES: [u64; 2] = [5, 15];
/// A failed revisions read is retried after this delay, doubling up to `READ_RETRY_MAX`.
const READ_RETRY: Duration = Duration::from_millis(500);
const READ_RETRY_MAX: Duration = Duration::from_secs(30);

pub(super) fn start(native: &NativeServices, cx: &mut Context<ApplicationView>) {
    let runtime = native.local.runtime.clone();
    cx.set_global(Developers {
        runtime: runtime.clone(),
        tools: native.local.developers.clone(),
    });
    let Some(cloud) = native.cloud.service.clone() else {
        return;
    };
    let api = CloudApi::new(
        cloud.clone(),
        native.local.storage_root.join("cloud-api.json"),
    );
    cx.set_global(api.clone());
    let mut identities = cloud.identities();
    let reply = runtime.watch_changes(CHANGES_SQL.into(), vec![]);
    let executor = cx.background_executor().clone();
    cx.spawn(async move |_, _| {
        let mut watch = match async { reply?.receive().await }.await {
            Ok(watch) => watch,
            Err(error) => {
                tracing::warn!(%error, "Cloud API revisions are not watched");
                return;
            }
        };
        let generation = Arc::new(AtomicU64::new(0));
        let mut user: Option<String> = None;
        let mut retry: Option<Duration> = None;
        loop {
            let current = identities
                .borrow_and_update()
                .account_id
                .as_deref()
                .map(str::to_owned);
            if current != user {
                user = current;
                if user.is_some() {
                    executor.spawn(api.initialize()).detach();
                }
            }
            // A failed refresh still signals a write; the full read below does not depend on it.
            while watch.errors.try_recv().is_ok() {}
            let sequence = watch.snapshots.borrow_and_update().sequence;
            // Sequence 0 is the placeholder before the first result; it is not an empty library.
            let rows = match user.as_ref().filter(|_| sequence > 0) {
                Some(_) => match async {
                    runtime
                        .read_rows(REVISIONS_SQL.into(), vec![])?
                        .receive()
                        .await
                }
                .await
                {
                    Ok(rows) => {
                        retry = None;
                        Some(rows)
                    }
                    // The write that prompted this read was consumed, so nothing else would
                    // read it again; retry until a read succeeds.
                    Err(error) => {
                        let delay =
                            retry.map_or(READ_RETRY, |delay| (delay * 2).min(READ_RETRY_MAX));
                        retry = Some(delay);
                        tracing::warn!(%error, ?delay, "Cloud API revisions could not be read");
                        None
                    }
                },
                None => {
                    retry = None;
                    None
                }
            };
            if let (Some(user), Some(rows)) = (user.as_ref(), rows) {
                let changes = api.observe(user, &revisions(&rows));
                if !changes.deletes.is_empty() {
                    executor.spawn(api.sync(user.clone(), true)).detach();
                }
                if !changes.upserts.is_empty() {
                    let current = generation.fetch_add(1, Ordering::SeqCst) + 1;
                    let (api, user, generation, timer) = (
                        api.clone(),
                        user.clone(),
                        generation.clone(),
                        executor.clone(),
                    );
                    executor
                        .spawn(async move {
                            timer.timer(UPLOAD_DELAY).await;
                            if generation.load(Ordering::SeqCst) != current {
                                return;
                            }
                            for delay in INVALID_RETRIES {
                                if !matches!(api.sync(user.clone(), false).await, Ok(true)) {
                                    return;
                                }
                                timer.timer(Duration::from_secs(delay)).await;
                            }
                            let _ = api.sync(user, false).await;
                        })
                        .detach();
                }
            }
            if watch.terminal_error().is_some() {
                break;
            }
            let changed = select(
                identities.changed().boxed(),
                watch.snapshots.changed().boxed(),
            );
            let woke = match retry {
                Some(delay) => match select(changed, executor.timer(delay)).await {
                    Either::Left((woke, _)) => woke,
                    Either::Right(_) => continue,
                },
                None => changed.await,
            };
            match woke {
                Either::Left((Ok(()), _)) | Either::Right((Ok(()), _)) => {}
                _ => break,
            }
        }
        let _ = watch.unsubscribe().await;
    })
    .detach();
}

fn revisions(rows: &[Value]) -> Vec<Revision> {
    rows.iter()
        .filter_map(|row| {
            Some(Revision {
                id: row["id"].as_str()?.to_owned(),
                deleted_at: row["deletedAt"].as_str().map(str::to_owned),
                revision: row["revision"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect()
}
