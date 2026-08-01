#[cfg(test)]
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backon::{BackoffBuilder, ExponentialBuilder};
use sqlx::pool::PoolConnection;
use sqlx::{Sqlite, SqlitePool};
use tokio::sync::oneshot;

use super::super::state::CloudsyncRuntimeState;
use super::super::types::CloudsyncNetworkResult;

pub(super) fn record_sync_result(
    runtime: &Mutex<CloudsyncRuntimeState>,
    result: CloudsyncNetworkResult,
    local_work_remaining: bool,
) {
    let mut runtime = runtime.lock().unwrap();
    runtime.last_sync = Some(result);

    if let Some(error) = runtime.last_sync.as_ref().and_then(embedded_sync_error) {
        runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
        runtime.last_error = Some(error);
        runtime.last_error_kind = runtime
            .last_error
            .as_deref()
            .map(|error| anlg_cloudsync::Error::Io(std::io::Error::other(error)).kind());
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
) {
    let mut runtime = runtime.lock().unwrap();
    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
    runtime.last_error = Some(error.to_string());
    runtime.last_error_kind = Some(error.kind());
}
const MAX_BACKOFF_SECS: u64 = 300;
pub(super) const CLOUDSYNC_PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

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
    pub(super) runtime_state: Arc<Mutex<CloudsyncRuntimeState>>,
    pub(super) sync_hook: Arc<Mutex<Option<Arc<dyn super::super::CloudsyncSyncHook>>>>,
    pub(super) config: CloudsyncLoopConfig,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum CloudsyncWake {
    Interval,
    Requested,
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

pub(super) async fn cloudsync_background_loop(
    context: CloudsyncLoopContext,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut next_sync_delay = cloudsync_next_delay(None, false, context.config.interval);
    let mut request_pending = false;
    loop {
        let wake = tokio::select! {
            biased;
            _ = &mut shutdown_rx => break,
            _ = context.sync_requested.notified() => CloudsyncWake::Requested,
            _ = tokio::time::sleep(next_sync_delay) => CloudsyncWake::Interval,
        };
        request_pending = cloudsync_request_pending(request_pending, wake);
        let Ok(sync_available) = context.sync_operation.try_lock() else {
            tracing::debug!("cloudsync interval skipped because another sync is active");
            next_sync_delay = cloudsync_busy_delay(request_pending, context.config.interval);
            continue;
        };
        drop(sync_available);
        request_pending = false;
        let Some(result) = sync_cloudsync_with_retry(&context, &mut shutdown_rx).await else {
            break;
        };

        match result {
            Ok(CloudsyncStepOutcome::Completed(step)) => {
                next_sync_delay = cloudsync_next_delay(
                    Some(&step.network),
                    step.local_work_remaining,
                    context.config.interval,
                );
                record_sync_result(
                    &context.runtime_state,
                    step.network,
                    step.local_work_remaining,
                );
            }
            Ok(CloudsyncStepOutcome::Deferred) => {
                next_sync_delay = context.config.interval;
            }
            Err(error) => {
                let kind = error.kind();
                let mut runtime = context.runtime_state.lock().unwrap();
                runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                runtime.last_error = Some(error.to_string());
                runtime.last_error_kind = Some(kind);
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

                let failures = {
                    let mut runtime = context.runtime_state.lock().unwrap();
                    runtime.consecutive_failures = runtime.consecutive_failures.saturating_add(1);
                    runtime.last_error = Some(error.to_string());
                    runtime.last_error_kind = Some(error.kind());
                    runtime.consecutive_failures
                };
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
    let pending_batch = {
        let mut connection = connection.lock().await;
        if connection.is_none() {
            *connection = Some(pool.acquire().await?);
        }
        let result =
            super::super::ops::ensure_pending_payload_fits(connection.as_mut().unwrap(), interrupt)
                .await;
        if pool.options().get_max_connections() == 1 {
            connection.take();
        }
        result
    };
    let pending_batch = match pending_batch {
        Err(_) if cloudsync_activity_paused(sync_hook) => {
            return Ok(CloudsyncStepOutcome::Deferred);
        }
        result => result?,
    };
    let directive = if pending_batch.chunks > 0 {
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
        super::super::CloudsyncSyncDirective::SendAndReceive if pending_batch.chunks == 0 => {
            let result = super::super::ops::ensure_pending_payload_fits(
                connection.as_mut().unwrap(),
                interrupt,
            )
            .await;
            match result {
                Err(_) if cloudsync_activity_paused(sync_hook) => {
                    if pool.options().get_max_connections() == 1 {
                        connection.take();
                    }
                    return Ok(CloudsyncStepOutcome::Deferred);
                }
                result => result?.chunks > 0,
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
    let send = match directive {
        super::super::CloudsyncSyncDirective::SendAndReceive => {
            super::super::ops::guarded_interruptible_network_send_changes(
                connection.as_mut().unwrap(),
                interrupt,
                || cloudsync_activity_paused(sync_hook),
            )
            .await
        }
        super::super::CloudsyncSyncDirective::ReceiveOnly => Ok(CloudsyncNetworkResult::default()),
        super::super::CloudsyncSyncDirective::Deferred => {
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
