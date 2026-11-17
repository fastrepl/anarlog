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

const UPLOAD_DELAY: Duration = Duration::from_millis(1500);
/// Shipping retries a snapshot the API rejected as invalid after these delays.
const INVALID_RETRIES: [u64; 2] = [5, 15];

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
    let reply = runtime.watch_query(REVISIONS_SQL.into(), vec![]);
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
            let mut failed = watch.terminal_error().is_some();
            while watch.errors.try_recv().is_ok() {
                failed = true;
            }
            let snapshot = watch.snapshots.borrow_and_update().clone();
            // Sequence 0 is the placeholder before the first result; it is not an empty library.
            if let Some(user) = user.as_ref().filter(|_| !failed && snapshot.sequence > 0) {
                let changes = api.observe(user, &revisions(&snapshot.rows));
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
            match select(
                identities.changed().boxed(),
                watch.snapshots.changed().boxed(),
            )
            .await
            {
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
