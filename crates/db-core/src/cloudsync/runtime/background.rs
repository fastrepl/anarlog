#[cfg(test)]
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{BackoffBuilder, ExponentialBuilder};
use sqlx::pool::PoolConnection;
use sqlx::{Sqlite, SqliteConnection, SqlitePool};
use tokio::sync::oneshot;

use super::super::state::CloudsyncRuntimeState;
use super::super::types::{
    CloudsyncActivityEntry, CloudsyncActivityStatus, CloudsyncActivityTrigger,
    CloudsyncNetworkResult,
};

pub(super) const MAX_ACTIVITY_LOG_ENTRIES: usize = 50;

pub(super) fn record_sync_result(
    runtime: &Mutex<CloudsyncRuntimeState>,
    result: CloudsyncNetworkResult,
    local_work_remaining: bool,
    trigger: CloudsyncActivityTrigger,
) {
    let (activity, transferred_data) =
        activity_entry_from_result(&result, local_work_remaining, trigger);
    let mut runtime = runtime.lock().unwrap();
    let should_record_activity = trigger == CloudsyncActivityTrigger::Manual
        || activity.status == CloudsyncActivityStatus::Failed
        || transferred_data
        || (activity.status == CloudsyncActivityStatus::Completed
            && runtime.activity_log.back().is_some_and(|previous| {
                matches!(
                    previous.status,
                    CloudsyncActivityStatus::Progress | CloudsyncActivityStatus::Failed
                )
            }));
    runtime.last_sync = Some(result);

    if let Some(error) = runtime.last_sync.as_ref().and_then(embedded_sync_error) {
        runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
        runtime.last_error = Some(error);
        runtime.last_error_kind = runtime
            .last_error
            .as_deref()
            .map(|error| anlg_cloudsync::Error::Io(std::io::Error::other(error)).kind());
        if should_record_activity {
            push_activity(&mut runtime, activity);
        }
        return;
    }

    if runtime
        .last_sync
        .as_ref()
        .is_some_and(|result| sync_result_settled(result) && !local_work_remaining)
    {
        runtime.last_sync_at_ms = Some(now_ms());
    }
    runtime.last_error = None;
    runtime.last_error_kind = None;
    runtime.consecutive_failures = 0;
    if should_record_activity {
        push_activity(&mut runtime, activity);
    }
}

fn activity_entry_from_result(
    result: &CloudsyncNetworkResult,
    local_work_remaining: bool,
    trigger: CloudsyncActivityTrigger,
) -> (CloudsyncActivityEntry, bool) {
    let error = embedded_sync_error(result);
    let sent_bytes = result
        .send
        .as_ref()
        .map_or(0, |send| send.bytes.max(0) as u64);
    let received_bytes = result
        .receive
        .as_ref()
        .map_or(0, |receive| receive.bytes.max(0) as u64);
    let received_rows = result
        .receive
        .as_ref()
        .map_or(0, |receive| receive.rows.max(0) as u64);
    let transferred_data = sent_bytes > 0 || received_bytes > 0 || received_rows > 0;
    let status = if error.is_some() {
        CloudsyncActivityStatus::Failed
    } else if sync_result_settled(result) && !local_work_remaining {
        CloudsyncActivityStatus::Completed
    } else {
        CloudsyncActivityStatus::Progress
    };

    (
        CloudsyncActivityEntry {
            timestamp_ms: now_ms(),
            trigger,
            status,
            sent_bytes,
            received_bytes,
            error,
        },
        transferred_data,
    )
}

fn push_activity(runtime: &mut CloudsyncRuntimeState, activity: CloudsyncActivityEntry) {
    if runtime.activity_log.len() >= MAX_ACTIVITY_LOG_ENTRIES {
        runtime.activity_log.pop_front();
    }
    runtime.activity_log.push_back(activity);
}

fn sync_result_settled(result: &CloudsyncNetworkResult) -> bool {
    sync_send_settled(result)
        && result.receive.as_ref().is_some_and(|receive| {
            receive.complete && receive.error.is_none() && receive.last_failure.is_none()
        })
}

fn sync_send_settled(result: &CloudsyncNetworkResult) -> bool {
    result.send.as_ref().is_none_or(|send| {
        send.status.eq_ignore_ascii_case("synced") && send.last_failure.is_none()
    })
}

pub(super) fn sync_result_needs_receive_progress(result: &CloudsyncNetworkResult) -> bool {
    embedded_sync_error(result).is_none()
        && result.receive.is_some()
        && !sync_result_settled(result)
}

fn sync_result_uploaded(result: &CloudsyncNetworkResult) -> bool {
    result.send.as_ref().is_some_and(|send| {
        send.status.eq_ignore_ascii_case("synced") && send.chunks > 0 && send.last_failure.is_none()
    })
}

pub(super) fn cloudsync_next_delay(
    result: Option<&CloudsyncNetworkResult>,
    local_work_remaining: bool,
    interval: Duration,
) -> Duration {
    match result {
        None => Duration::ZERO,
        Some(result) if sync_result_needs_receive_progress(result) => CLOUDSYNC_PROGRESS_INTERVAL,
        Some(result) if local_work_remaining && !sync_result_uploaded(result) => {
            CLOUDSYNC_PROGRESS_INTERVAL
        }
        Some(_) => interval,
    }
}

fn embedded_sync_error(result: &CloudsyncNetworkResult) -> Option<String> {
    let mut errors = Vec::new();

    if let Some(send) = &result.send {
        if !send.status.eq_ignore_ascii_case("synced")
            && !send.status.eq_ignore_ascii_case("syncing")
        {
            errors.push(format!("send status: {}", send.status));
        }
        if let Some(last_failure) = &send.last_failure {
            errors.push(format!("send failure: {last_failure}"));
        }
    }

    if let Some(receive) = &result.receive {
        if let Some(error) = &receive.error {
            errors.push(format!("receive error: {error}"));
        }
        if let Some(last_failure) = &receive.last_failure {
            errors.push(format!("receive failure: {last_failure}"));
        }
    }

    (!errors.is_empty()).then(|| errors.join("; "))
}

pub(super) fn record_sync_error(
    runtime: &Mutex<CloudsyncRuntimeState>,
    error: &anlg_cloudsync::Error,
    trigger: CloudsyncActivityTrigger,
) -> u32 {
    let mut runtime = runtime.lock().unwrap();
    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
    runtime.last_error = Some(error.to_string());
    runtime.last_error_kind = Some(error.kind());
    push_activity(
        &mut runtime,
        CloudsyncActivityEntry {
            timestamp_ms: now_ms(),
            trigger,
            status: CloudsyncActivityStatus::Failed,
            sent_bytes: 0,
            received_bytes: 0,
            error: Some(error.to_string()),
        },
    );
    runtime.consecutive_failures
}
const MAX_BACKOFF_SECS: u64 = 300;
pub(super) const CLOUDSYNC_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
pub(super) const CLOUDSYNC_CHANGE_DEBOUNCE: Duration = Duration::from_secs(1);

pub(super) struct CloudsyncStepResult {
    pub(super) network: CloudsyncNetworkResult,
    pub(super) local_work_remaining: bool,
}

pub(super) enum CloudsyncStepOutcome {
    Completed(Box<CloudsyncStepResult>),
    Deferred,
}

#[derive(Clone, Copy)]
pub(super) struct CloudsyncLoopConfig {
    pub(super) interval: Duration,
}

pub(super) struct CloudsyncLoopContext {
    pub(super) pool: SqlitePool,
    pub(super) connection: Arc<tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>>,
    pub(super) interrupt: Arc<super::super::CloudsyncInterruptHandle>,
    pub(super) sync_operation: Arc<tokio::sync::Mutex<()>>,
    pub(super) sync_requested: Arc<tokio::sync::Notify>,
    pub(super) change_rx: tokio::sync::broadcast::Receiver<anlg_db_change::TableChange>,
    pub(super) synced_tables: std::collections::HashSet<String>,
    pub(super) runtime_state: Arc<Mutex<CloudsyncRuntimeState>>,
    pub(super) sync_hook: Arc<Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>>,
    pub(super) config: CloudsyncLoopConfig,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CloudsyncWake {
    Interval,
    Requested,
    Changed,
}

pub(super) fn cloudsync_request_pending(request_pending: bool, wake: CloudsyncWake) -> bool {
    request_pending || wake == CloudsyncWake::Requested
}

pub(super) fn cloudsync_busy_delay(request_pending: bool, interval: Duration) -> Duration {
    if request_pending {
        CLOUDSYNC_PROGRESS_INTERVAL
    } else {
        interval
    }
}

pub(super) fn cloudsync_wake_deadline(
    next_sync_deadline: tokio::time::Instant,
    change_deadline: Option<tokio::time::Instant>,
) -> tokio::time::Instant {
    change_deadline.map_or(next_sync_deadline, |deadline| {
        deadline.min(next_sync_deadline)
    })
}

/// Completes when a commit touches a CloudSync-relevant table. Irrelevant
/// tables are skipped without completing so a pending interval timer keeps
/// its deadline; a lagged subscription wakes conservatively.
pub(super) async fn next_synced_change(
    change_rx: &mut tokio::sync::broadcast::Receiver<anlg_db_change::TableChange>,
    synced_tables: &std::collections::HashSet<String>,
    closed: &mut bool,
) {
    if *closed {
        return std::future::pending().await;
    }
    loop {
        match change_rx.recv().await {
            Ok(change) => {
                if synced_tables.contains(&change.table.to_ascii_lowercase()) {
                    return;
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => return,
            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                *closed = true;
                return std::future::pending().await;
            }
        }
    }
}

/// Change events that arrive while a sync is in flight mix echoes of that
/// sync (applied remote rows, hook reconciliation) with real concurrent
/// local commits, and the two are indistinguishable. Drain the queue so
/// echoes cannot immediately schedule another round, but report whether a
/// synced-table event was dropped so the caller can debounce one follow-up
/// round; when the events were only echoes that round finds nothing,
/// writes nothing, and the loop settles. A lagged subscription reports
/// conservatively.
pub(super) fn drain_pending_changes(
    change_rx: &mut tokio::sync::broadcast::Receiver<anlg_db_change::TableChange>,
    synced_tables: &std::collections::HashSet<String>,
) -> bool {
    let mut synced_change_seen = false;
    loop {
        match change_rx.try_recv() {
            Ok(change) => {
                synced_change_seen |= synced_tables.contains(&change.table.to_ascii_lowercase());
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                synced_change_seen = true;
            }
            Err(_) => return synced_change_seen,
        }
    }
}

pub(super) async fn cloudsync_background_loop(
    mut context: CloudsyncLoopContext,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut next_sync_deadline =
        tokio::time::Instant::now() + cloudsync_next_delay(None, false, context.config.interval);
    let mut change_deadline: Option<tokio::time::Instant> = None;
    let mut request_pending = false;
    let mut change_rx_closed = false;
    loop {
        let wake = tokio::select! {
            biased;
            _ = &mut shutdown_rx => break,
            _ = context.sync_requested.notified() => CloudsyncWake::Requested,
            _ = tokio::time::sleep_until(cloudsync_wake_deadline(next_sync_deadline, change_deadline)) => CloudsyncWake::Interval,
            _ = next_synced_change(&mut context.change_rx, &context.synced_tables, &mut change_rx_closed) => CloudsyncWake::Changed,
        };
        if wake == CloudsyncWake::Changed {
            change_deadline = Some(tokio::time::Instant::now() + CLOUDSYNC_CHANGE_DEBOUNCE);
            continue;
        }
        request_pending = cloudsync_request_pending(request_pending, wake);
        let Ok(sync_available) = context.sync_operation.try_lock() else {
            tracing::debug!("cloudsync interval skipped because another sync is active");
            let now = tokio::time::Instant::now();
            next_sync_deadline =
                now + cloudsync_busy_delay(request_pending, context.config.interval);
            if change_deadline.is_some() {
                change_deadline = Some(now + CLOUDSYNC_PROGRESS_INTERVAL);
            }
            continue;
        };
        drop(sync_available);
        request_pending = false;
        change_deadline = None;
        let Some(result) = sync_cloudsync_with_retry(&context, &mut shutdown_rx).await else {
            break;
        };
        if drain_pending_changes(&mut context.change_rx, &context.synced_tables) {
            change_deadline = Some(tokio::time::Instant::now() + CLOUDSYNC_CHANGE_DEBOUNCE);
        }

        match result {
            Ok(CloudsyncStepOutcome::Completed(step)) => {
                next_sync_deadline = tokio::time::Instant::now()
                    + cloudsync_next_delay(
                        Some(&step.network),
                        step.local_work_remaining,
                        context.config.interval,
                    );
                record_sync_result(
                    &context.runtime_state,
                    step.network,
                    step.local_work_remaining,
                    CloudsyncActivityTrigger::Background,
                );
            }
            Ok(CloudsyncStepOutcome::Deferred) => {
                next_sync_deadline = tokio::time::Instant::now() + context.config.interval;
            }
            Err(error) => {
                record_sync_error(
                    &context.runtime_state,
                    &error,
                    CloudsyncActivityTrigger::Background,
                );
                let mut runtime = context.runtime_state.lock().unwrap();
                runtime.running = false;
                break;
            }
        }
    }
}

async fn sync_cloudsync_with_retry(
    context: &CloudsyncLoopContext,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<Result<CloudsyncStepOutcome, anlg_cloudsync::Error>> {
    let mut backoff = ExponentialBuilder::default()
        .with_min_delay(context.config.interval)
        .with_max_delay(Duration::from_secs(MAX_BACKOFF_SECS))
        .with_jitter()
        .build();

    loop {
        let result = run_sync_or_shutdown(context, shutdown_rx).await?;

        match result {
            Err(error) if error.kind() == anlg_cloudsync::ErrorKind::Transient => {
                let Some(retry_after) = backoff.next() else {
                    return Some(Err(error));
                };

                let failures = record_sync_error(
                    &context.runtime_state,
                    &error,
                    CloudsyncActivityTrigger::Background,
                );
                tracing::warn!(
                    error = %error,
                    retry_after = ?retry_after,
                    failures,
                    "cloudsync transient error, retrying",
                );

                if !wait_for_retry_request_or_shutdown(
                    retry_after,
                    &context.sync_requested,
                    shutdown_rx,
                )
                .await
                {
                    return None;
                }
            }
            result => return Some(result),
        }
    }
}

async fn run_sync_or_shutdown(
    context: &CloudsyncLoopContext,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<Result<CloudsyncStepOutcome, anlg_cloudsync::Error>> {
    let sync = sync_cloudsync_connection(
        &context.pool,
        &context.connection,
        &context.interrupt,
        &context.sync_operation,
        &context.runtime_state,
        &context.sync_hook,
    );
    tokio::pin!(sync);

    tokio::select! {
        biased;
        _ = &mut *shutdown_rx => {
            cancel_active_sync_hook(&context.sync_hook);
            let mut interrupt_interval = tokio::time::interval(Duration::from_millis(25));
            loop {
                tokio::select! {
                    biased;
                    _ = &mut sync => return None,
                    _ = interrupt_interval.tick() => {
                        cancel_active_sync_hook(&context.sync_hook);
                        context.interrupt.interrupt();
                    }
                }
            }
        }
        result = &mut sync => Some(result),
    }
}

#[cfg(test)]
pub(super) async fn run_or_shutdown<T>(
    future: impl Future<Output = T>,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> Option<T> {
    tokio::select! {
        biased;
        _ = &mut *shutdown_rx => None,
        result = future => Some(result),
    }
}

pub(super) async fn wait_for_retry_request_or_shutdown(
    retry_after: Duration,
    sync_requested: &tokio::sync::Notify,
    shutdown_rx: &mut oneshot::Receiver<()>,
) -> bool {
    tokio::select! {
        biased;
        _ = &mut *shutdown_rx => false,
        _ = sync_requested.notified() => true,
        _ = tokio::time::sleep(retry_after) => true,
    }
}

pub(super) async fn sync_cloudsync_connection(
    pool: &SqlitePool,
    connection: &tokio::sync::Mutex<Option<PoolConnection<Sqlite>>>,
    interrupt: &super::super::CloudsyncInterruptHandle,
    sync_operation: &tokio::sync::Mutex<()>,
    runtime_state: &Mutex<CloudsyncRuntimeState>,
    sync_hook: &Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>,
) -> Result<CloudsyncStepOutcome, anlg_cloudsync::Error> {
    let _sync_operation = sync_operation.lock().await;
    if cloudsync_activity_paused(sync_hook) {
        tracing::debug!("cloudsync deferred while local activity is active");
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let pending_batch_exists = {
        let mut connection = connection.lock().await;
        if connection.is_none() {
            *connection = Some(pool.acquire().await?);
        }
        let result =
            pending_cloudsync_payload_exists(connection.as_mut().unwrap(), interrupt).await;
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        result
    };
    let pending_batch_exists = match pending_batch_exists {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    let directive = if pending_batch_exists {
        super::super::CloudsyncSyncDirective::SendAndReceive
    } else {
        run_before_sync_hook(sync_hook, pool).await?
    };
    if directive == super::super::CloudsyncSyncDirective::Deferred {
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    if cloudsync_activity_paused(sync_hook) {
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let mut connection = connection.lock().await;
    if connection.is_none() {
        *connection = Some(pool.acquire().await?);
    }
    let has_outbound_work = match directive {
        super::super::CloudsyncSyncDirective::SendAndReceive if !pending_batch_exists => {
            let result =
                pending_cloudsync_payload_exists(connection.as_mut().unwrap(), interrupt).await;
            match result {
                Err(_) if cloudsync_activity_paused(sync_hook) => {
                    if pool.options().get_max_connections() == 1 {
                        connection.take();
                    }
                    return Ok(CloudsyncStepOutcome::Deferred);
                }
                result => result?,
            }
        }
        super::super::CloudsyncSyncDirective::SendAndReceive => true,
        super::super::CloudsyncSyncDirective::ReceiveOnly => false,
        super::super::CloudsyncSyncDirective::Deferred => {
            unreachable!("deferred before native sync")
        }
    };
    runtime_state.lock().unwrap().outbound_work_state = Some(has_outbound_work);
    if cloudsync_activity_paused(sync_hook) {
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let send = match (directive, has_outbound_work) {
        (super::super::CloudsyncSyncDirective::SendAndReceive, true) => {
            super::super::ops::guarded_interruptible_network_send_changes(
                connection.as_mut().unwrap(),
                interrupt,
                || cloudsync_activity_paused(sync_hook),
            )
            .await
        }
        (
            super::super::CloudsyncSyncDirective::SendAndReceive
            | super::super::CloudsyncSyncDirective::ReceiveOnly,
            false,
        ) => Ok(CloudsyncNetworkResult::default()),
        (super::super::CloudsyncSyncDirective::ReceiveOnly, true) => {
            unreachable!("receive-only sync cannot have outbound work")
        }
        (super::super::CloudsyncSyncDirective::Deferred, _) => {
            unreachable!("deferred before native sync")
        }
    };
    let send = match send {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            if pool.options().get_max_connections() == 1 {
                connection.take();
            }
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    if sync_send_settled(&send) {
        runtime_state.lock().unwrap().outbound_work_state = Some(false);
    }
    if cloudsync_activity_paused(sync_hook) {
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    let receive = super::super::ops::interruptible_network_receive_changes(
        connection.as_mut().unwrap(),
        interrupt,
    )
    .await;
    let receive = match receive {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            if pool.options().get_max_connections() == 1 {
                connection.take();
            }
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    let result = merge_bounded_sync_results(send, receive);
    if pool.options().get_max_connections() == 1 {
        connection.take();
    }
    drop(connection);
    let outcome = run_after_sync_hook(sync_hook, pool, &result).await?;
    if outcome.deferred || cloudsync_activity_paused(sync_hook) {
        return Ok(CloudsyncStepOutcome::Deferred);
    }
    runtime_state.lock().unwrap().outbound_work_state = Some(outcome.local_work_remaining);
    Ok(CloudsyncStepOutcome::Completed(Box::new(
        CloudsyncStepResult {
            network: result,
            local_work_remaining: outcome.local_work_remaining,
        },
    )))
}

pub(super) async fn pending_cloudsync_payload_exists(
    connection: &mut SqliteConnection,
    interrupt: &super::super::CloudsyncInterruptHandle,
) -> Result<bool, anlg_cloudsync::Error> {
    if !super::super::ops::cloudsync_has_local_unsent_changes_on(&mut *connection).await? {
        return Ok(false);
    }
    Ok(
        super::super::ops::ensure_pending_payload_fits(connection, interrupt)
            .await?
            .chunks
            > 0,
    )
}

pub(in crate::cloudsync) fn cloudsync_activity_paused(
    hook: &Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>,
) -> bool {
    hook.lock()
        .unwrap()
        .as_ref()
        .is_some_and(|hook| hook.activity_paused())
}

pub(super) fn cancel_active_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>,
) {
    if let Some(hook) = hook.lock().unwrap().clone() {
        hook.cancel_active_sync();
    }
}

pub(super) fn merge_bounded_sync_results(
    send: CloudsyncNetworkResult,
    receive: CloudsyncNetworkResult,
) -> CloudsyncNetworkResult {
    CloudsyncNetworkResult {
        send: send.send.or(receive.send),
        receive: receive.receive.or(send.receive),
    }
}

pub(super) async fn run_before_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>,
    pool: &SqlitePool,
) -> Result<super::super::CloudsyncSyncDirective, anlg_cloudsync::Error> {
    let hook = hook.lock().unwrap().clone();
    match hook {
        Some(hook) => hook.before_sync(pool).await,
        None => Ok(super::super::CloudsyncSyncDirective::default()),
    }
}

pub(super) async fn run_after_sync_hook(
    hook: &Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>,
    pool: &SqlitePool,
    result: &CloudsyncNetworkResult,
) -> Result<super::super::CloudsyncHookOutcome, anlg_cloudsync::Error> {
    let hook = hook.lock().unwrap().clone();
    match hook {
        Some(hook) => hook.after_sync(pool, result).await,
        None => Ok(super::super::CloudsyncHookOutcome::default()),
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
