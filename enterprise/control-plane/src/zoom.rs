use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anarlog_enterprise_zoom_rtms_worker::{
    ZoomRtmsCredentials, ZoomRtmsSession, ZoomRtmsSessionConfig, ZoomRtmsSessionError,
    ZoomRtmsSessionOutcome, ZoomRtmsStarted, ZoomRtmsTerminal, ZoomRtmsWebhookEvent,
    ZoomWebhookValidationResponse, ZoomWebhookVerificationError,
};
use anlg_meeting_capture::{
    BotState, CaptureEvent, CaptureEventPayload, CaptureProviderKind, ProviderMetadata,
    TerminalReason, TerminalReasonKind, TransitionError,
};
use async_trait::async_trait;
use chrono::Utc;
use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, mpsc, oneshot, watch};

use crate::{
    capture::{CaptureDispatch, CaptureJobCheckpoint, CaptureJobLeaseIdentity},
    config::ZoomConfig,
    store::{ControlPlaneStore, StoreError},
};

const CAPTURE_LEASE_DURATION: Duration = Duration::from_secs(60);
const CAPTURE_LEASE_RENEW_INTERVAL: Duration = Duration::from_secs(20);
const TRANSCRIPT_CHANNEL_CAPACITY: usize = 32;
const STORE_RETRY_ATTEMPTS: usize = 3;
const STORE_RETRY_INITIAL_DELAY: Duration = Duration::from_millis(25);
const STORE_RETRY_MAX_DELAY: Duration = Duration::from_millis(100);
const DISPATCH_RECOVERY_INTERVAL: Duration = Duration::from_secs(20);
const STOP_WEBHOOK_GRACE_PERIOD: Duration = Duration::from_secs(2);
const RECONNECT_BASE_DELAY: Duration = Duration::from_secs(3);
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(30);
const RECONNECT_WINDOW: Duration = Duration::from_secs(55);
const PENDING_STOP_TTL: Duration = Duration::from_secs(5 * 60);
const MAX_PENDING_STOPS: usize = 1024;

static WORKER_NONCE: AtomicU64 = AtomicU64::new(0);

#[async_trait]
pub trait ZoomWebhookDispatcher: Send + Sync {
    async fn start(
        &self,
        workspace_id: &str,
        started: ZoomRtmsStarted,
    ) -> Result<ZoomDispatchOutcome, ZoomDispatchError>;

    async fn stop(
        &self,
        terminal: &ZoomRtmsTerminal,
        reason: ZoomStopReason,
    ) -> Result<(), ZoomDispatchError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoomDispatchOutcome {
    pub job_id: String,
    pub started: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoomStopReason {
    Stopped,
    Interrupted,
}

impl ZoomStopReason {
    fn merge(self, other: Self) -> Self {
        if self == Self::Stopped || other == Self::Stopped {
            Self::Stopped
        } else {
            Self::Interrupted
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ZoomDispatchError {
    #[error("no dispatchable capture job matches the Zoom meeting")]
    NotFound,
    #[error("Zoom capture dispatch is unavailable")]
    Unavailable(#[source] StoreError),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ZoomWebhookOutcome {
    Validation(ZoomWebhookValidationResponse),
    Accepted(ZoomDispatchOutcome),
    Ignored,
}

#[derive(Debug, thiserror::Error)]
pub enum ZoomWebhookError {
    #[error("Zoom webhook headers are missing")]
    MissingHeaders,
    #[error(transparent)]
    Verification(#[from] ZoomWebhookVerificationError),
    #[error(transparent)]
    Protocol(#[from] anarlog_enterprise_zoom_rtms_worker::ZoomRtmsProtocolError),
    #[error(transparent)]
    Dispatch(#[from] ZoomDispatchError),
    #[error("system clock is before the Unix epoch")]
    InvalidSystemClock,
}

#[derive(Clone)]
pub struct ZoomWebhookService {
    config: ZoomConfig,
    dispatcher: Arc<dyn ZoomWebhookDispatcher>,
}

impl ZoomWebhookService {
    pub fn new(config: ZoomConfig, dispatcher: Arc<dyn ZoomWebhookDispatcher>) -> Self {
        Self { config, dispatcher }
    }

    pub async fn handle(
        &self,
        timestamp: Option<&str>,
        signature: Option<&str>,
        body: &[u8],
    ) -> Result<ZoomWebhookOutcome, ZoomWebhookError> {
        let timestamp = timestamp.ok_or(ZoomWebhookError::MissingHeaders)?;
        let signature = signature.ok_or(ZoomWebhookError::MissingHeaders)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ZoomWebhookError::InvalidSystemClock)?
            .as_secs();
        self.handle_at(timestamp, signature, body, now).await
    }

    pub async fn handle_at(
        &self,
        timestamp: &str,
        signature: &str,
        body: &[u8],
        now_unix_seconds: u64,
    ) -> Result<ZoomWebhookOutcome, ZoomWebhookError> {
        self.config
            .verifier()
            .verify(timestamp, signature, body, now_unix_seconds)?;
        match ZoomRtmsWebhookEvent::parse(body)? {
            ZoomRtmsWebhookEvent::UrlValidation { plain_token } => {
                Ok(ZoomWebhookOutcome::Validation(
                    self.config.verifier().validation_response(plain_token)?,
                ))
            }
            ZoomRtmsWebhookEvent::Started(started) => {
                let Some(workspace_id) = self.config.workspace_for_account(&started.account_id)
                else {
                    return Ok(ZoomWebhookOutcome::Ignored);
                };
                match self.dispatcher.start(workspace_id, started).await {
                    Ok(outcome) => Ok(ZoomWebhookOutcome::Accepted(outcome)),
                    Err(ZoomDispatchError::NotFound) => Ok(ZoomWebhookOutcome::Ignored),
                    Err(error) => Err(error.into()),
                }
            }
            ZoomRtmsWebhookEvent::Stopped(terminal) => {
                self.dispatcher
                    .stop(&terminal, ZoomStopReason::Stopped)
                    .await?;
                Ok(ZoomWebhookOutcome::Ignored)
            }
            ZoomRtmsWebhookEvent::Interrupted(terminal) => {
                self.dispatcher
                    .stop(&terminal, ZoomStopReason::Interrupted)
                    .await?;
                Ok(ZoomWebhookOutcome::Ignored)
            }
            ZoomRtmsWebhookEvent::Other { .. } => Ok(ZoomWebhookOutcome::Ignored),
        }
    }
}

#[derive(Clone)]
pub struct ZoomCaptureDispatcher {
    store: Arc<dyn ControlPlaneStore>,
    credentials: ZoomRtmsCredentials,
    registry: Arc<Mutex<ZoomCaptureRegistry>>,
}

#[derive(Default)]
struct ZoomCaptureRegistry {
    tasks: HashMap<String, ZoomCaptureTask>,
    pending_stops: HashMap<String, PendingZoomStop>,
}

struct ZoomCaptureTask {
    workspace_id: String,
    job_id: String,
    meeting_uuid: String,
    commands: mpsc::UnboundedSender<ZoomCaptureCommand>,
}

struct PendingZoomStop {
    meeting_uuid: String,
    reason: ZoomStopReason,
    received_at: Instant,
}

enum ZoomCaptureCommand {
    Stop(ZoomStopReason),
    Restart {
        started: ZoomRtmsStarted,
        accepted: oneshot::Sender<()>,
    },
}

enum ZoomCaptureInstruction {
    Stop(ZoomStopReason),
    Restart(ZoomRtmsStarted),
}

impl ZoomCaptureRegistry {
    fn prune_pending_stops(&mut self, now: Instant) {
        self.pending_stops
            .retain(|_, stop| now.saturating_duration_since(stop.received_at) <= PENDING_STOP_TTL);
    }
}

fn accept_zoom_command(command: ZoomCaptureCommand) -> Option<ZoomCaptureInstruction> {
    match command {
        ZoomCaptureCommand::Stop(reason) => Some(ZoomCaptureInstruction::Stop(reason)),
        ZoomCaptureCommand::Restart { started, accepted } => accepted
            .send(())
            .ok()
            .map(|()| ZoomCaptureInstruction::Restart(started)),
    }
}

fn try_receive_zoom_instruction(
    commands: &mut mpsc::UnboundedReceiver<ZoomCaptureCommand>,
) -> Result<Option<ZoomCaptureInstruction>, ZoomCaptureWorkerError> {
    loop {
        match commands.try_recv() {
            Ok(command) => {
                if let Some(instruction) = accept_zoom_command(command) {
                    return Ok(Some(instruction));
                }
            }
            Err(mpsc::error::TryRecvError::Empty) => return Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                return Err(ZoomCaptureWorkerError::ControlChannelClosed);
            }
        }
    }
}

async fn receive_zoom_instruction(
    commands: &mut mpsc::UnboundedReceiver<ZoomCaptureCommand>,
) -> Result<ZoomCaptureInstruction, ZoomCaptureWorkerError> {
    loop {
        let command = commands
            .recv()
            .await
            .ok_or(ZoomCaptureWorkerError::ControlChannelClosed)?;
        if let Some(instruction) = accept_zoom_command(command) {
            return Ok(instruction);
        }
    }
}

impl ZoomCaptureDispatcher {
    pub fn new(store: Arc<dyn ControlPlaneStore>, credentials: ZoomRtmsCredentials) -> Self {
        Self {
            store,
            credentials,
            registry: Arc::new(Mutex::new(ZoomCaptureRegistry::default())),
        }
    }

    pub async fn recover_pending(&self) -> Result<(), ZoomDispatchError> {
        let dispatches = self
            .store
            .list_capture_dispatches(CaptureProviderKind::ZoomRtms)
            .await
            .map_err(dispatch_store_error)?;
        for dispatch in dispatches {
            let workspace_id = dispatch.workspace_id.clone();
            let job_id = dispatch.job_id.clone();
            if let Err(error) = self.recover_dispatch(dispatch).await {
                tracing::error!(%workspace_id, %job_id, error = %error, "failed to recover durable Zoom dispatch");
            }
        }
        Ok(())
    }

    async fn recover_dispatch(&self, dispatch: CaptureDispatch) -> Result<(), ZoomDispatchError> {
        let started = serde_json::from_value::<ZoomRtmsStarted>(dispatch.payload)
            .map_err(StoreError::from)
            .map_err(dispatch_store_error)?;
        if started.stream_id != dispatch.dispatch_id {
            return Err(ZoomDispatchError::Unavailable(
                StoreError::InvalidCaptureEvent(
                    "durable Zoom dispatch ID does not match its payload".into(),
                ),
            ));
        }
        let checkpoint = match self
            .store
            .read_capture_checkpoint(&dispatch.workspace_id, &dispatch.job_id)
            .await
        {
            Ok(checkpoint) => checkpoint,
            Err(StoreError::NotFound) | Err(StoreError::CaptureJobTerminal) => return Ok(()),
            Err(error) => return Err(dispatch_store_error(error)),
        };
        validate_checkpoint(&checkpoint, &started).map_err(|_| {
            ZoomDispatchError::Unavailable(StoreError::InvalidCaptureEvent(
                "durable Zoom dispatch does not match its capture checkpoint".into(),
            ))
        })?;
        self.start_checkpoint(&dispatch.workspace_id, checkpoint, started)
            .await?;
        Ok(())
    }

    pub fn spawn_recovery(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval_at(
                tokio::time::Instant::now() + DISPATCH_RECOVERY_INTERVAL,
                DISPATCH_RECOVERY_INTERVAL,
            );
            loop {
                interval.tick().await;
                if let Err(error) = self.recover_pending().await {
                    tracing::error!(error = %error, "failed to recover durable Zoom dispatches");
                }
            }
        });
    }

    async fn start_checkpoint(
        &self,
        workspace_id: &str,
        checkpoint: CaptureJobCheckpoint,
        started: ZoomRtmsStarted,
    ) -> Result<ZoomDispatchOutcome, ZoomDispatchError> {
        {
            let mut registry = self.registry.lock().await;
            registry.prune_pending_stops(Instant::now());
            if let Some(task) = registry.tasks.get(&started.stream_id) {
                if task.commands.is_closed() {
                    registry.tasks.remove(&started.stream_id);
                } else {
                    return Ok(ZoomDispatchOutcome {
                        job_id: task.job_id.clone(),
                        started: false,
                    });
                }
            }
        }
        let (worker_id, lease_id) = worker_lease_ids(&checkpoint.job.job_id, &started.stream_id);
        let lease = match self
            .store
            .claim_capture_job(
                workspace_id,
                &checkpoint.job.job_id,
                &worker_id,
                &lease_id,
                CAPTURE_LEASE_DURATION,
            )
            .await
        {
            Ok(lease) => lease,
            Err(StoreError::CaptureLeaseUnavailable) | Err(StoreError::CaptureJobTerminal) => {
                return Ok(ZoomDispatchOutcome {
                    job_id: checkpoint.job.job_id,
                    started: false,
                });
            }
            Err(error) => return Err(dispatch_store_error(error)),
        };
        let lease = CaptureJobLeaseIdentity {
            worker_id: lease.worker_id,
            lease_id: lease.lease_id,
            epoch: lease.epoch,
        };
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let stream_id = started.stream_id.clone();
        let pending_stop = {
            let mut registry = self.registry.lock().await;
            let pending_stop = registry.pending_stops.remove(&stream_id);
            registry.tasks.insert(
                stream_id.clone(),
                ZoomCaptureTask {
                    workspace_id: workspace_id.to_string(),
                    job_id: checkpoint.job.job_id.clone(),
                    meeting_uuid: started.meeting_uuid.clone(),
                    commands: command_tx.clone(),
                },
            );
            pending_stop
        };
        if let Some(pending) = pending_stop
            && pending.meeting_uuid == started.meeting_uuid
        {
            let _ = command_tx.send(ZoomCaptureCommand::Stop(pending.reason));
        }

        let store = self.store.clone();
        let credentials = self.credentials.clone();
        let registry = self.registry.clone();
        let cleanup_commands = command_tx.clone();
        let job_id = checkpoint.job.job_id.clone();
        let response_job_id = job_id.clone();
        let workspace_id = workspace_id.to_string();
        tokio::spawn(async move {
            let result = run_zoom_capture(
                store,
                &workspace_id,
                checkpoint,
                lease,
                credentials,
                started,
                command_rx,
            )
            .await;
            if let Err(error) = result {
                tracing::error!(%workspace_id, %job_id, error = %error, "Zoom RTMS capture worker stopped");
            }
            let mut registry = registry.lock().await;
            if registry
                .tasks
                .get(&stream_id)
                .is_some_and(|task| task.commands.same_channel(&cleanup_commands))
            {
                registry.tasks.remove(&stream_id);
            }
        });

        Ok(ZoomDispatchOutcome {
            job_id: response_job_id,
            started: true,
        })
    }
}

#[async_trait]
impl ZoomWebhookDispatcher for ZoomCaptureDispatcher {
    async fn start(
        &self,
        workspace_id: &str,
        started: ZoomRtmsStarted,
    ) -> Result<ZoomDispatchOutcome, ZoomDispatchError> {
        let active = {
            let mut registry = self.registry.lock().await;
            registry.prune_pending_stops(Instant::now());
            registry.tasks.get(&started.stream_id).map(|task| {
                (
                    task.workspace_id.clone(),
                    task.job_id.clone(),
                    task.meeting_uuid.clone(),
                    task.commands.clone(),
                )
            })
        };
        if let Some((active_workspace_id, job_id, meeting_uuid, commands)) = active {
            if active_workspace_id != workspace_id || meeting_uuid != started.meeting_uuid {
                return Err(ZoomDispatchError::Unavailable(
                    StoreError::InvalidCaptureEvent(
                        "active Zoom stream does not match its signed webhook".into(),
                    ),
                ));
            }
            let checkpoint = self
                .store
                .read_capture_checkpoint(workspace_id, &job_id)
                .await
                .map_err(dispatch_store_error)?;
            validate_checkpoint(&checkpoint, &started).map_err(|_| {
                ZoomDispatchError::Unavailable(StoreError::InvalidCaptureEvent(
                    "active Zoom dispatch does not match its signed webhook".into(),
                ))
            })?;
            self.store
                .save_capture_dispatch(&capture_dispatch(workspace_id, &job_id, &started)?)
                .await
                .map_err(dispatch_store_error)?;
            let (accepted_tx, accepted_rx) = oneshot::channel();
            if commands
                .send(ZoomCaptureCommand::Restart {
                    started: started.clone(),
                    accepted: accepted_tx,
                })
                .is_ok()
                && accepted_rx.await.is_ok()
            {
                return Ok(ZoomDispatchOutcome {
                    job_id,
                    started: false,
                });
            }
            let mut registry = self.registry.lock().await;
            if registry
                .tasks
                .get(&started.stream_id)
                .is_some_and(|task| task.commands.same_channel(&commands))
            {
                registry.tasks.remove(&started.stream_id);
            }
        }

        let existing_dispatch = self
            .store
            .find_capture_dispatch(CaptureProviderKind::ZoomRtms, &started.stream_id)
            .await;
        match existing_dispatch {
            Ok(dispatch) => {
                if dispatch.workspace_id != workspace_id {
                    return Err(ZoomDispatchError::NotFound);
                }
                let checkpoint = self
                    .store
                    .read_capture_checkpoint(&dispatch.workspace_id, &dispatch.job_id)
                    .await
                    .map_err(dispatch_store_error)?;
                validate_checkpoint(&checkpoint, &started).map_err(|_| {
                    ZoomDispatchError::Unavailable(StoreError::InvalidCaptureEvent(
                        "durable Zoom dispatch does not match its signed webhook".into(),
                    ))
                })?;
                self.store
                    .save_capture_dispatch(&capture_dispatch(
                        &dispatch.workspace_id,
                        &dispatch.job_id,
                        &started,
                    )?)
                    .await
                    .map_err(dispatch_store_error)?;
                return self
                    .start_checkpoint(&dispatch.workspace_id, checkpoint, started)
                    .await;
            }
            Err(StoreError::NotFound) => {}
            Err(error) => return Err(dispatch_store_error(error)),
        }

        let mut external_ids = vec![started.meeting_id.clone()];
        if started.meeting_uuid != started.meeting_id {
            external_ids.push(started.meeting_uuid.clone());
        }
        let checkpoint = self
            .store
            .find_dispatchable_capture_checkpoint(
                workspace_id,
                CaptureProviderKind::ZoomRtms,
                &external_ids,
            )
            .await
            .map_err(dispatch_store_error)?;
        validate_checkpoint(&checkpoint, &started).map_err(|_| {
            ZoomDispatchError::Unavailable(StoreError::InvalidCaptureEvent(
                "Zoom capture checkpoint does not match its signed webhook".into(),
            ))
        })?;
        let dispatch = capture_dispatch(workspace_id, &checkpoint.job.job_id, &started)?;
        self.store
            .save_capture_dispatch(&dispatch)
            .await
            .map_err(dispatch_store_error)?;
        self.start_checkpoint(workspace_id, checkpoint, started)
            .await
    }

    async fn stop(
        &self,
        terminal: &ZoomRtmsTerminal,
        reason: ZoomStopReason,
    ) -> Result<(), ZoomDispatchError> {
        let active = {
            let mut registry = self.registry.lock().await;
            registry.prune_pending_stops(Instant::now());
            registry.tasks.get(&terminal.stream_id).and_then(|task| {
                (task.meeting_uuid == terminal.meeting_uuid).then(|| task.commands.clone())
            })
        };
        if let Some(commands) = active
            && commands.send(ZoomCaptureCommand::Stop(reason)).is_ok()
        {
            return Ok(());
        }

        let dispatch = match self
            .store
            .find_capture_dispatch(CaptureProviderKind::ZoomRtms, &terminal.stream_id)
            .await
        {
            Ok(dispatch) => dispatch,
            Err(StoreError::NotFound) => return Ok(()),
            Err(error) => return Err(dispatch_store_error(error)),
        };
        let started = serde_json::from_value::<ZoomRtmsStarted>(dispatch.payload)
            .map_err(StoreError::from)
            .map_err(dispatch_store_error)?;
        if started.meeting_uuid != terminal.meeting_uuid {
            return Ok(());
        }

        let mut registry = self.registry.lock().await;
        let now = Instant::now();
        registry.prune_pending_stops(now);
        if let Some(task) = registry.tasks.get(&terminal.stream_id) {
            if task.meeting_uuid == terminal.meeting_uuid {
                let _ = task.commands.send(ZoomCaptureCommand::Stop(reason));
            }
            return Ok(());
        }
        if let Some(pending) = registry.pending_stops.get_mut(&terminal.stream_id) {
            if pending.meeting_uuid == terminal.meeting_uuid {
                pending.reason = pending.reason.merge(reason);
                pending.received_at = now;
            }
            return Ok(());
        }
        if registry.pending_stops.len() >= MAX_PENDING_STOPS {
            return Err(ZoomDispatchError::Unavailable(StoreError::OutOfRange(
                "pending Zoom stop capacity",
            )));
        }
        registry.pending_stops.insert(
            terminal.stream_id.clone(),
            PendingZoomStop {
                meeting_uuid: terminal.meeting_uuid.clone(),
                reason,
                received_at: now,
            },
        );
        Ok(())
    }
}

fn capture_dispatch(
    workspace_id: &str,
    job_id: &str,
    started: &ZoomRtmsStarted,
) -> Result<CaptureDispatch, ZoomDispatchError> {
    Ok(CaptureDispatch {
        workspace_id: workspace_id.to_string(),
        job_id: job_id.to_string(),
        provider: CaptureProviderKind::ZoomRtms,
        dispatch_id: started.stream_id.clone(),
        payload: serde_json::to_value(started)
            .map_err(StoreError::from)
            .map_err(dispatch_store_error)?,
    })
}

fn dispatch_store_error(error: StoreError) -> ZoomDispatchError {
    match error {
        StoreError::NotFound => ZoomDispatchError::NotFound,
        error => ZoomDispatchError::Unavailable(error),
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_zoom_capture(
    store: Arc<dyn ControlPlaneStore>,
    workspace_id: &str,
    checkpoint: CaptureJobCheckpoint,
    lease: CaptureJobLeaseIdentity,
    credentials: ZoomRtmsCredentials,
    mut started: ZoomRtmsStarted,
    mut commands: mpsc::UnboundedReceiver<ZoomCaptureCommand>,
) -> Result<(), ZoomCaptureWorkerError> {
    let mut events = ZoomCaptureEvents::resume(&checkpoint)?;
    if events.state == BotState::Stopping {
        return persist_zoom_stop(
            store.as_ref(),
            workspace_id,
            &checkpoint.job.job_id,
            &lease,
            &mut events,
        )
        .await;
    }
    let mut reconnect = None;
    loop {
        if let Some(state) = reconnect.as_mut() {
            let reconnect_result = wait_for_zoom_reconnect(
                store.as_ref(),
                workspace_id,
                &checkpoint.job.job_id,
                &lease,
                &mut commands,
                state,
            )
            .await;
            let reconnect_command = match reconnect_result {
                Ok(command) => command,
                Err(error) => {
                    return fail_zoom_capture(
                        store.as_ref(),
                        workspace_id,
                        &checkpoint.job.job_id,
                        &lease,
                        &mut events,
                        error,
                    )
                    .await;
                }
            };
            match reconnect_command {
                Some(ZoomCaptureInstruction::Stop(ZoomStopReason::Stopped)) => {
                    return persist_zoom_stop(
                        store.as_ref(),
                        workspace_id,
                        &checkpoint.job.job_id,
                        &lease,
                        &mut events,
                    )
                    .await;
                }
                Some(ZoomCaptureInstruction::Stop(ZoomStopReason::Interrupted)) => {
                    schedule_zoom_interruption(&mut reconnect);
                    continue;
                }
                Some(ZoomCaptureInstruction::Restart(restarted)) => {
                    started = restarted;
                    reconnect = Some(ZoomReconnectState::immediate());
                    continue;
                }
                None => {}
            }
        }

        let transcript_sequence = match store
            .next_capture_transcript_sequence(workspace_id, &checkpoint.job.job_id)
            .await
        {
            Ok(sequence) => sequence,
            Err(error) => {
                return fail_zoom_capture(
                    store.as_ref(),
                    workspace_id,
                    &checkpoint.job.job_id,
                    &lease,
                    &mut events,
                    error.into(),
                )
                .await;
            }
        };
        let mut connected = false;
        let result = run_zoom_session(
            store.as_ref(),
            workspace_id,
            &checkpoint.job.job_id,
            &lease,
            &credentials,
            started.clone(),
            transcript_sequence,
            &mut commands,
            &mut events,
            &mut connected,
        )
        .await;
        if connected {
            reconnect = None;
        }

        let command = match result {
            Ok(ZoomSessionRunOutcome::Command(command)) => Some(command),
            Ok(ZoomSessionRunOutcome::Session(ZoomRtmsSessionOutcome::StoppedByRequest)) => {
                return Err(ZoomCaptureWorkerError::ControlChannelClosed);
            }
            Err(error) => {
                if !connected && reconnect.is_some() && error.can_reconnect() {
                    continue;
                }
                let command = if error.can_reconcile_with_stop() {
                    tokio::time::timeout(
                        STOP_WEBHOOK_GRACE_PERIOD,
                        receive_zoom_instruction(&mut commands),
                    )
                    .await
                    .ok()
                    .and_then(Result::ok)
                } else {
                    None
                };
                if command.is_none() {
                    return fail_zoom_capture(
                        store.as_ref(),
                        workspace_id,
                        &checkpoint.job.job_id,
                        &lease,
                        &mut events,
                        error,
                    )
                    .await;
                }
                command
            }
        };

        match command.expect("Zoom session command exists") {
            ZoomCaptureInstruction::Stop(ZoomStopReason::Stopped) => {
                return persist_zoom_stop(
                    store.as_ref(),
                    workspace_id,
                    &checkpoint.job.job_id,
                    &lease,
                    &mut events,
                )
                .await;
            }
            ZoomCaptureInstruction::Stop(ZoomStopReason::Interrupted) => {
                schedule_zoom_interruption(&mut reconnect);
            }
            ZoomCaptureInstruction::Restart(restarted) => {
                started = restarted;
                reconnect = Some(ZoomReconnectState::immediate());
            }
        }
    }
}

async fn fail_zoom_capture(
    store: &dyn ControlPlaneStore,
    workspace_id: &str,
    job_id: &str,
    lease: &CaptureJobLeaseIdentity,
    events: &mut ZoomCaptureEvents,
    error: ZoomCaptureWorkerError,
) -> Result<(), ZoomCaptureWorkerError> {
    if !events.state.is_terminal() {
        let failed = events.transition(
            BotState::Failed,
            Some(TerminalReason {
                kind: TerminalReasonKind::ProviderError,
                message: Some("Zoom RTMS worker exited before the stream stopped".into()),
                retryable: true,
            }),
        )?;
        if let Err(store_error) = append(store, workspace_id, job_id, lease, events, failed).await {
            tracing::warn!(%workspace_id, %job_id, error = %store_error, "failed to persist Zoom worker terminal state");
        }
    }
    Err(error)
}

struct ZoomReconnectState {
    deadline: tokio::time::Instant,
    attempt: u32,
    immediate: bool,
}

impl ZoomReconnectState {
    fn backoff() -> Self {
        Self {
            deadline: tokio::time::Instant::now() + RECONNECT_WINDOW,
            attempt: 0,
            immediate: false,
        }
    }

    fn immediate() -> Self {
        Self {
            immediate: true,
            ..Self::backoff()
        }
    }
}

fn schedule_zoom_interruption(reconnect: &mut Option<ZoomReconnectState>) {
    if reconnect.is_none() {
        *reconnect = Some(ZoomReconnectState::backoff());
    }
}

async fn wait_for_zoom_reconnect(
    store: &dyn ControlPlaneStore,
    workspace_id: &str,
    job_id: &str,
    lease: &CaptureJobLeaseIdentity,
    commands: &mut mpsc::UnboundedReceiver<ZoomCaptureCommand>,
    reconnect: &mut ZoomReconnectState,
) -> Result<Option<ZoomCaptureInstruction>, ZoomCaptureWorkerError> {
    renew_lease(store, workspace_id, job_id, lease).await?;
    let now = tokio::time::Instant::now();
    if now >= reconnect.deadline {
        return Err(ZoomCaptureWorkerError::ReconnectWindowExpired);
    }
    let delay = if reconnect.immediate {
        reconnect.immediate = false;
        Duration::ZERO
    } else {
        let multiplier = 1_u32
            .checked_shl(reconnect.attempt.min(31))
            .unwrap_or(u32::MAX);
        reconnect.attempt = reconnect.attempt.saturating_add(1);
        RECONNECT_BASE_DELAY
            .saturating_mul(multiplier)
            .min(RECONNECT_MAX_DELAY)
    };
    let sleep_until = (now + delay).min(reconnect.deadline);
    tokio::select! {
        _ = tokio::time::sleep_until(sleep_until) => {
            if tokio::time::Instant::now() >= reconnect.deadline {
                Err(ZoomCaptureWorkerError::ReconnectWindowExpired)
            } else {
                Ok(None)
            }
        }
        command = receive_zoom_instruction(commands) => command.map(Some),
    }
}

async fn persist_zoom_stop(
    store: &dyn ControlPlaneStore,
    workspace_id: &str,
    job_id: &str,
    lease: &CaptureJobLeaseIdentity,
    events: &mut ZoomCaptureEvents,
) -> Result<(), ZoomCaptureWorkerError> {
    if events.state == BotState::Queued {
        let event = events.transition(
            BotState::Canceled,
            Some(TerminalReason {
                kind: TerminalReasonKind::StoppedByRequest,
                message: None,
                retryable: false,
            }),
        )?;
        append(store, workspace_id, job_id, lease, events, event).await
    } else {
        if events.state != BotState::Stopping {
            let event = events.transition(BotState::Stopping, None)?;
            append(store, workspace_id, job_id, lease, events, event).await?;
        }
        let event = events.transition(
            BotState::Completed,
            Some(TerminalReason {
                kind: TerminalReasonKind::MeetingEnded,
                message: None,
                retryable: false,
            }),
        )?;
        append(store, workspace_id, job_id, lease, events, event).await
    }
}

enum ZoomSessionRunOutcome {
    Command(ZoomCaptureInstruction),
    Session(ZoomRtmsSessionOutcome),
}

#[allow(clippy::too_many_arguments)]
async fn run_zoom_session(
    store: &dyn ControlPlaneStore,
    workspace_id: &str,
    job_id: &str,
    lease: &CaptureJobLeaseIdentity,
    credentials: &ZoomRtmsCredentials,
    started: ZoomRtmsStarted,
    transcript_sequence: u64,
    commands: &mut mpsc::UnboundedReceiver<ZoomCaptureCommand>,
    events: &mut ZoomCaptureEvents,
    connected: &mut bool,
) -> Result<ZoomSessionRunOutcome, ZoomCaptureWorkerError> {
    if let Some(command) = try_receive_zoom_instruction(commands)? {
        return Ok(ZoomSessionRunOutcome::Command(command));
    }
    if events.state == BotState::Queued {
        let event = events.transition(BotState::Launching, None)?;
        append(store, workspace_id, job_id, lease, events, event).await?;
    }
    if !matches!(
        events.state,
        BotState::Launching | BotState::Joined | BotState::Capturing
    ) {
        return Err(ZoomCaptureWorkerError::InvalidCheckpoint);
    }
    let mut connection = Box::pin(ZoomRtmsSession::connect(
        credentials,
        started,
        ZoomRtmsSessionConfig {
            initial_segment_sequence: transcript_sequence,
            ..ZoomRtmsSessionConfig::default()
        },
    ));
    let mut session = tokio::select! {
        result = &mut connection => result?,
        command = receive_zoom_instruction(commands) => {
            return command.map(ZoomSessionRunOutcome::Command);
        }
    };
    drop(connection);
    *connected = true;
    if let Some(command) = try_receive_zoom_instruction(commands)? {
        session.shutdown().await;
        return Ok(ZoomSessionRunOutcome::Command(command));
    }
    if events.state == BotState::Launching {
        let event = events.transition(BotState::Joined, None)?;
        append(store, workspace_id, job_id, lease, events, event).await?;
    }
    if let Some(command) = try_receive_zoom_instruction(commands)? {
        session.shutdown().await;
        return Ok(ZoomSessionRunOutcome::Command(command));
    }
    if events.state == BotState::Joined {
        let event = events.transition(BotState::Capturing, None)?;
        append(store, workspace_id, job_id, lease, events, event).await?;
    }

    let (transcript_tx, mut transcript_rx) = mpsc::channel(TRANSCRIPT_CHANNEL_CAPACITY);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut stream = tokio::spawn(async move {
        let result = session.stream_transcripts(transcript_tx, shutdown_rx).await;
        session.shutdown().await;
        result
    });
    let mut renewals = tokio::time::interval_at(
        tokio::time::Instant::now() + CAPTURE_LEASE_RENEW_INTERVAL,
        CAPTURE_LEASE_RENEW_INTERVAL,
    );
    let mut transcripts_open = true;
    let result = async {
        let outcome = loop {
            tokio::select! {
                result = &mut stream => {
                    break ZoomSessionRunOutcome::Session(
                        result.map_err(ZoomCaptureWorkerError::StreamTask)??
                    );
                }
                command = receive_zoom_instruction(commands) => {
                    let command = command?;
                    let _ = shutdown_tx.send(true);
                    let outcome = (&mut stream)
                        .await
                        .map_err(ZoomCaptureWorkerError::StreamTask)??;
                    debug_assert_eq!(outcome, ZoomRtmsSessionOutcome::StoppedByRequest);
                    break ZoomSessionRunOutcome::Command(command);
                }
                transcript = transcript_rx.recv(), if transcripts_open => {
                    if let Some(transcript) = transcript {
                        let event = events.payload(CaptureEventPayload::Transcript(transcript))?;
                        append(store, workspace_id, job_id, lease, events, event).await?;
                    } else {
                        transcripts_open = false;
                    }
                }
                _ = renewals.tick() => {
                    renew_lease(store, workspace_id, job_id, lease).await?;
                }
            }
        };
        while let Ok(transcript) = transcript_rx.try_recv() {
            let event = events.payload(CaptureEventPayload::Transcript(transcript))?;
            append(store, workspace_id, job_id, lease, events, event).await?;
        }
        Ok(outcome)
    }
    .await;
    if result.is_err() && !stream.is_finished() {
        stream.abort();
        let _ = stream.await;
    }
    result
}

async fn append(
    store: &dyn ControlPlaneStore,
    workspace_id: &str,
    job_id: &str,
    lease: &CaptureJobLeaseIdentity,
    events: &mut ZoomCaptureEvents,
    event: CaptureEvent,
) -> Result<(), ZoomCaptureWorkerError> {
    let mut retry_delay = STORE_RETRY_INITIAL_DELAY;
    for attempt in 1..=STORE_RETRY_ATTEMPTS {
        match store
            .append_capture_event(workspace_id, job_id, lease, &event)
            .await
        {
            Ok(_) => {
                events.commit(&event)?;
                return Ok(());
            }
            Err(error) if attempt < STORE_RETRY_ATTEMPTS && retryable_store_error(&error) => {
                tokio::time::sleep(retry_delay).await;
                retry_delay = retry_delay.saturating_mul(2).min(STORE_RETRY_MAX_DELAY);
            }
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("store retry policy always performs at least one attempt")
}

async fn renew_lease(
    store: &dyn ControlPlaneStore,
    workspace_id: &str,
    job_id: &str,
    lease: &CaptureJobLeaseIdentity,
) -> Result<(), ZoomCaptureWorkerError> {
    let mut retry_delay = STORE_RETRY_INITIAL_DELAY;
    for attempt in 1..=STORE_RETRY_ATTEMPTS {
        match store
            .renew_capture_job_lease(workspace_id, job_id, lease, CAPTURE_LEASE_DURATION)
            .await
        {
            Ok(_) => return Ok(()),
            Err(error) if attempt < STORE_RETRY_ATTEMPTS && retryable_store_error(&error) => {
                tokio::time::sleep(retry_delay).await;
                retry_delay = retry_delay.saturating_mul(2).min(STORE_RETRY_MAX_DELAY);
            }
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("store retry policy always performs at least one attempt")
}

fn retryable_store_error(error: &StoreError) -> bool {
    matches!(error, StoreError::Database(_))
}

struct ZoomCaptureEvents {
    bot_id: String,
    state: BotState,
    next_sequence: u64,
}

impl ZoomCaptureEvents {
    fn resume(checkpoint: &CaptureJobCheckpoint) -> Result<Self, ZoomCaptureWorkerError> {
        if checkpoint.state.is_terminal() || checkpoint.state == BotState::WaitingForAdmission {
            return Err(ZoomCaptureWorkerError::InvalidCheckpoint);
        }
        Ok(Self {
            bot_id: checkpoint.job.bot_id.clone(),
            state: checkpoint.state,
            next_sequence: checkpoint.next_sequence,
        })
    }

    fn transition(
        &self,
        next: BotState,
        reason: Option<TerminalReason>,
    ) -> Result<CaptureEvent, ZoomCaptureWorkerError> {
        let transition = self.state.transition_to(next, reason)?;
        self.payload(CaptureEventPayload::Lifecycle(transition))
    }

    fn payload(
        &self,
        payload: CaptureEventPayload,
    ) -> Result<CaptureEvent, ZoomCaptureWorkerError> {
        let sequence = self.next_sequence;
        self.next_sequence
            .checked_add(1)
            .ok_or(ZoomCaptureWorkerError::SequenceExhausted)?;
        Ok(CaptureEvent {
            id: format!("capture-event-{sequence}"),
            bot_id: self.bot_id.clone(),
            sequence,
            occurred_at: Utc::now(),
            payload,
            metadata: ProviderMetadata::default(),
        })
    }

    fn commit(&mut self, event: &CaptureEvent) -> Result<(), ZoomCaptureWorkerError> {
        if event.bot_id != self.bot_id || event.sequence != self.next_sequence {
            return Err(ZoomCaptureWorkerError::InvalidCheckpoint);
        }
        if let CaptureEventPayload::Lifecycle(transition) = &event.payload {
            if transition.from != self.state {
                return Err(ZoomCaptureWorkerError::InvalidCheckpoint);
            }
            self.state = transition.to;
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(ZoomCaptureWorkerError::SequenceExhausted)?;
        Ok(())
    }
}

fn validate_checkpoint(
    checkpoint: &CaptureJobCheckpoint,
    started: &ZoomRtmsStarted,
) -> Result<(), ZoomCaptureWorkerError> {
    let worker = anlg_meeting_capture::CaptureWorkerCheckpoint {
        job_id: checkpoint.job.job_id.clone(),
        bot_id: checkpoint.job.bot_id.clone(),
        provider: checkpoint.job.provider,
        meeting: checkpoint.job.meeting.clone(),
        state: checkpoint.state,
        next_sequence: checkpoint.next_sequence,
    };
    worker
        .validate()
        .map_err(|_| ZoomCaptureWorkerError::InvalidCheckpoint)?;
    let external_id = checkpoint
        .job
        .meeting
        .external_id
        .as_deref()
        .ok_or(ZoomCaptureWorkerError::InvalidCheckpoint)?;
    if checkpoint.job.provider != CaptureProviderKind::ZoomRtms
        || (external_id != started.meeting_id && external_id != started.meeting_uuid)
    {
        return Err(ZoomCaptureWorkerError::InvalidCheckpoint);
    }
    Ok(())
}

fn worker_lease_ids(job_id: &str, stream_id: &str) -> (String, String) {
    let nonce = WORKER_NONCE.fetch_add(1, Ordering::Relaxed);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let process_id = std::process::id();
    let digest = Sha256::digest(
        format!("{job_id}\0{stream_id}\0{process_id}\0{timestamp}\0{nonce}").as_bytes(),
    );
    let suffix = digest
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    (
        format!("zoom-worker-{suffix}"),
        format!("zoom-lease-{suffix}"),
    )
}

#[derive(Debug, thiserror::Error)]
enum ZoomCaptureWorkerError {
    #[error("Zoom capture checkpoint is invalid")]
    InvalidCheckpoint,
    #[error("Zoom capture event sequence was exhausted")]
    SequenceExhausted,
    #[error(transparent)]
    Transition(#[from] TransitionError),
    #[error(transparent)]
    Session(#[from] ZoomRtmsSessionError),
    #[error("Zoom RTMS stream task stopped unexpectedly")]
    StreamTask(#[source] tokio::task::JoinError),
    #[error("Zoom RTMS capture control channel closed")]
    ControlChannelClosed,
    #[error("Zoom RTMS reconnect window expired")]
    ReconnectWindowExpired,
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ZoomCaptureWorkerError {
    fn can_reconcile_with_stop(&self) -> bool {
        matches!(
            self,
            Self::Session(
                ZoomRtmsSessionError::ConnectionClosed(_)
                    | ZoomRtmsSessionError::Websocket(_)
                    | ZoomRtmsSessionError::HandshakeTimeout(_)
            )
        )
    }

    fn can_reconnect(&self) -> bool {
        matches!(
            self,
            Self::Session(
                ZoomRtmsSessionError::Websocket(_)
                    | ZoomRtmsSessionError::HandshakeTimeout(_)
                    | ZoomRtmsSessionError::ConnectionClosed(_)
            )
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::{CaptureJob, CaptureJobLease, CaptureJobStatus, ProjectionPublication};
    use anlg_meeting_capture::{MeetingPlatform, MeetingReference};
    use anlg_session_ingest::{AcknowledgeRequest, DeliveryPage, SessionRead};
    use std::sync::{Mutex as StdMutex, atomic::AtomicUsize};
    use tokio::sync::Notify;

    struct TestStore {
        checkpoint: CaptureJobCheckpoint,
        events: StdMutex<Vec<CaptureEvent>>,
        dispatches: StdMutex<Vec<CaptureDispatch>>,
        append_failures_remaining: AtomicUsize,
        launching: Notify,
        terminal: Notify,
    }

    impl TestStore {
        fn new(checkpoint: CaptureJobCheckpoint) -> Self {
            Self {
                checkpoint,
                events: StdMutex::new(Vec::new()),
                dispatches: StdMutex::new(Vec::new()),
                append_failures_remaining: AtomicUsize::new(0),
                launching: Notify::new(),
                terminal: Notify::new(),
            }
        }

        fn with_append_failures(checkpoint: CaptureJobCheckpoint, failures: usize) -> Self {
            let store = Self::new(checkpoint);
            store
                .append_failures_remaining
                .store(failures, Ordering::SeqCst);
            store
        }
    }

    #[async_trait]
    impl ControlPlaneStore for TestStore {
        async fn readiness(&self) -> Result<(), StoreError> {
            Ok(())
        }

        async fn create_capture_job(
            &self,
            job: &CaptureJob,
        ) -> Result<CaptureJobStatus, StoreError> {
            Ok(CaptureJobStatus {
                job_id: job.job_id.clone(),
                created: true,
                state: BotState::Queued,
            })
        }

        async fn read_capture_checkpoint(
            &self,
            _workspace_id: &str,
            _job_id: &str,
        ) -> Result<CaptureJobCheckpoint, StoreError> {
            Ok(self.checkpoint.clone())
        }

        async fn find_dispatchable_capture_checkpoint(
            &self,
            workspace_id: &str,
            provider: CaptureProviderKind,
            external_ids: &[String],
        ) -> Result<CaptureJobCheckpoint, StoreError> {
            let external_id = self.checkpoint.job.meeting.external_id.as_ref().unwrap();
            if workspace_id == self.checkpoint.job.workspace_id
                && provider == self.checkpoint.job.provider
                && external_ids.contains(external_id)
            {
                Ok(self.checkpoint.clone())
            } else {
                Err(StoreError::NotFound)
            }
        }

        async fn save_capture_dispatch(
            &self,
            dispatch: &CaptureDispatch,
        ) -> Result<(), StoreError> {
            let mut current = self.dispatches.lock().unwrap();
            if let Some(existing) = current
                .iter_mut()
                .find(|existing| existing.job_id == dispatch.job_id)
            {
                return if existing.dispatch_id == dispatch.dispatch_id {
                    *existing = dispatch.clone();
                    Ok(())
                } else {
                    Err(StoreError::CaptureDispatchConflict)
                };
            }
            current.push(dispatch.clone());
            Ok(())
        }

        async fn list_capture_dispatches(
            &self,
            provider: CaptureProviderKind,
        ) -> Result<Vec<CaptureDispatch>, StoreError> {
            Ok(self
                .dispatches
                .lock()
                .unwrap()
                .iter()
                .filter(|dispatch| dispatch.provider == provider)
                .cloned()
                .collect())
        }

        async fn find_capture_dispatch(
            &self,
            provider: CaptureProviderKind,
            dispatch_id: &str,
        ) -> Result<CaptureDispatch, StoreError> {
            self.dispatches
                .lock()
                .unwrap()
                .iter()
                .find(|dispatch| {
                    dispatch.provider == provider && dispatch.dispatch_id == dispatch_id
                })
                .cloned()
                .ok_or(StoreError::NotFound)
        }

        async fn next_capture_transcript_sequence(
            &self,
            _workspace_id: &str,
            _job_id: &str,
        ) -> Result<u64, StoreError> {
            self.events
                .lock()
                .unwrap()
                .iter()
                .filter_map(|event| match &event.payload {
                    CaptureEventPayload::Transcript(transcript) => Some(transcript.sequence),
                    _ => None,
                })
                .try_fold(0, |next_sequence, sequence| {
                    Ok(next_sequence.max(
                        sequence
                            .checked_add(1)
                            .ok_or(StoreError::OutOfRange("transcript sequence"))?,
                    ))
                })
        }

        async fn claim_capture_job(
            &self,
            _workspace_id: &str,
            _job_id: &str,
            worker_id: &str,
            lease_id: &str,
            lease_duration: Duration,
        ) -> Result<CaptureJobLease, StoreError> {
            Ok(CaptureJobLease {
                worker_id: worker_id.into(),
                lease_id: lease_id.into(),
                epoch: 1,
                expires_at: Utc::now() + chrono::Duration::from_std(lease_duration).unwrap(),
            })
        }

        async fn renew_capture_job_lease(
            &self,
            _workspace_id: &str,
            _job_id: &str,
            lease: &CaptureJobLeaseIdentity,
            lease_duration: Duration,
        ) -> Result<CaptureJobLease, StoreError> {
            Ok(CaptureJobLease {
                worker_id: lease.worker_id.clone(),
                lease_id: lease.lease_id.clone(),
                epoch: lease.epoch,
                expires_at: Utc::now() + chrono::Duration::from_std(lease_duration).unwrap(),
            })
        }

        async fn append_capture_event(
            &self,
            _workspace_id: &str,
            _job_id: &str,
            _lease: &CaptureJobLeaseIdentity,
            event: &CaptureEvent,
        ) -> Result<ProjectionPublication, StoreError> {
            if self
                .append_failures_remaining
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(StoreError::Database(sqlx::Error::PoolTimedOut));
            }
            let mut events = self.events.lock().unwrap();
            events.push(event.clone());
            let envelope = crate::projector::project(&self.checkpoint.job, &events)
                .map_err(|error| StoreError::InvalidCaptureEvent(error.to_string()))?;
            let finalized = envelope.finalized;
            let revision = envelope.revision;
            let content_hash = crate::projector::content_hash(&envelope)?;
            if matches!(
                &event.payload,
                CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Launching
            ) {
                self.launching.notify_one();
            }
            if matches!(
                &event.payload,
                CaptureEventPayload::Lifecycle(transition) if transition.to.is_terminal()
            ) {
                self.dispatches.lock().unwrap().clear();
                self.terminal.notify_one();
            }
            Ok(ProjectionPublication {
                job_id: self.checkpoint.job.job_id.clone(),
                revision,
                finalized,
                content_hash,
                envelope,
            })
        }

        async fn list_deliveries(
            &self,
            _workspace_id: &str,
            _consumer_id: &str,
            after: u64,
            _limit: u16,
        ) -> Result<DeliveryPage, StoreError> {
            Ok(DeliveryPage {
                items: Vec::new(),
                next_cursor: after,
                has_more: false,
            })
        }

        async fn acknowledge(
            &self,
            _workspace_id: &str,
            _job_id: &str,
            _request: &AcknowledgeRequest,
        ) -> Result<(), StoreError> {
            Ok(())
        }

        async fn read_session(
            &self,
            _workspace_id: &str,
            _job_id: &str,
        ) -> Result<SessionRead, StoreError> {
            Err(StoreError::NotFound)
        }

        async fn get_capture_policy(
            &self,
            workspace_id: &str,
        ) -> Result<crate::schedule::CapturePolicy, StoreError> {
            Ok(crate::schedule::CapturePolicy::default_off(workspace_id))
        }

        async fn upsert_capture_policy(
            &self,
            policy: &crate::schedule::CapturePolicy,
        ) -> Result<crate::schedule::CapturePolicy, StoreError> {
            Ok(policy.clone())
        }

        async fn upsert_calendar_events(
            &self,
            _workspace_id: &str,
            _events: &[crate::schedule::CalendarEventInput],
        ) -> Result<Vec<crate::schedule::ScheduledCapture>, StoreError> {
            Ok(Vec::new())
        }

        async fn list_scheduled_captures(
            &self,
            _workspace_id: &str,
        ) -> Result<Vec<crate::schedule::ScheduledCapture>, StoreError> {
            Ok(Vec::new())
        }

        async fn cancel_scheduled_capture(
            &self,
            _workspace_id: &str,
            _calendar_event_id: &str,
        ) -> Result<crate::schedule::ScheduledCapture, StoreError> {
            Err(StoreError::NotFound)
        }

        async fn dispatch_due_scheduled_captures(
            &self,
            _now: chrono::DateTime<Utc>,
        ) -> Result<Vec<CaptureJobStatus>, StoreError> {
            Ok(Vec::new())
        }
    }

    fn zoom_checkpoint(external_id: &str) -> CaptureJobCheckpoint {
        CaptureJobCheckpoint {
            job: CaptureJob {
                workspace_id: "workspace-a".into(),
                job_id: "job-a".into(),
                bot_id: "bot-a".into(),
                owner_user_id: "owner-a".into(),
                requesting_actor_id: "actor-a".into(),
                session_id: "session-a".into(),
                session_title: "Zoom".into(),
                provider: CaptureProviderKind::ZoomRtms,
                meeting: MeetingReference {
                    platform: MeetingPlatform::Zoom,
                    url: "https://zoom.us/j/123".into(),
                    external_id: Some(external_id.into()),
                    calendar_event_id: None,
                },
                created_at: Utc::now(),
            },
            state: BotState::Queued,
            next_sequence: 0,
        }
    }

    async fn terminal_state_after_pending_stops(reasons: &[ZoomStopReason]) -> BotState {
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        let started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "meeting-uuid".into(),
            meeting_id: "123".into(),
            stream_id: "stream-id".into(),
            signaling_url: "wss://127.0.0.1:1/signaling".parse().unwrap(),
            event_timestamp_ms: 1,
        };
        store
            .save_capture_dispatch(&capture_dispatch("workspace-a", "job-a", &started).unwrap())
            .await
            .unwrap();
        let terminal = ZoomRtmsTerminal {
            meeting_uuid: "meeting-uuid".into(),
            stream_id: "stream-id".into(),
            event_timestamp_ms: 1,
        };
        for reason in reasons {
            dispatcher.stop(&terminal, *reason).await.unwrap();
        }
        dispatcher.start("workspace-a", started).await.unwrap();

        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("worker did not persist the pending terminal state");
        let events = store.events.lock().unwrap();
        match &events.last().unwrap().payload {
            CaptureEventPayload::Lifecycle(transition) => transition.to,
            _ => panic!("expected a terminal lifecycle event"),
        }
    }

    #[test]
    fn creates_valid_unique_lease_identities() {
        let first = worker_lease_ids("job-a", "stream-a");
        let second = worker_lease_ids("job-a", "stream-a");

        assert_ne!(first, second);
        for value in [first.0, first.1, second.0, second.1] {
            assert!(value.len() <= 128);
            assert!(
                value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            );
        }
    }

    #[test]
    fn duplicate_interruptions_keep_the_original_reconnect_deadline() {
        let mut reconnect = None;
        schedule_zoom_interruption(&mut reconnect);
        let deadline = reconnect.as_ref().unwrap().deadline;
        reconnect.as_mut().unwrap().attempt = 3;

        schedule_zoom_interruption(&mut reconnect);

        let reconnect = reconnect.unwrap();
        assert_eq!(reconnect.deadline, deadline);
        assert_eq!(reconnect.attempt, 3);
    }

    #[test]
    fn rejects_a_signed_meeting_that_does_not_match_the_job() {
        let checkpoint = zoom_checkpoint("123");
        let started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "uuid-b".into(),
            meeting_id: "456".into(),
            stream_id: "stream-a".into(),
            signaling_url: "wss://rtms.zoom.us/signaling".parse().unwrap(),
            event_timestamp_ms: 1,
        };

        assert!(validate_checkpoint(&checkpoint, &started).is_err());
    }

    #[test]
    fn connection_failures_can_reconcile_with_a_clean_stop() {
        assert!(
            ZoomCaptureWorkerError::Session(ZoomRtmsSessionError::ConnectionClosed("media"))
                .can_reconcile_with_stop()
        );
        assert!(
            !ZoomCaptureWorkerError::Session(ZoomRtmsSessionError::TranscriptBackpressure)
                .can_reconcile_with_stop()
        );
    }

    #[tokio::test]
    async fn skips_a_corrupt_dispatch_and_recovers_the_next_after_registry_loss() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "meeting-uuid".into(),
            meeting_id: "123".into(),
            stream_id: "stream-id".into(),
            signaling_url: format!("wss://{address}/signaling").parse().unwrap(),
            event_timestamp_ms: 1,
        };
        store
            .save_capture_dispatch(&CaptureDispatch {
                workspace_id: "workspace-a".into(),
                job_id: "job-a".into(),
                provider: CaptureProviderKind::ZoomRtms,
                dispatch_id: started.stream_id.clone(),
                payload: serde_json::to_value(&started).unwrap(),
            })
            .await
            .unwrap();
        store.dispatches.lock().unwrap().insert(
            0,
            CaptureDispatch {
                workspace_id: "workspace-corrupt".into(),
                job_id: "job-corrupt".into(),
                provider: CaptureProviderKind::ZoomRtms,
                dispatch_id: "stream-corrupt".into(),
                payload: serde_json::json!({"invalid": true}),
            },
        );
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );

        dispatcher.recover_pending().await.unwrap();

        tokio::time::timeout(Duration::from_secs(5), store.terminal.notified())
            .await
            .expect("recovered worker did not terminalize");
        let states = store
            .events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match &event.payload {
                CaptureEventPayload::Lifecycle(transition) => Some(transition.to),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(states, vec![BotState::Launching, BotState::Failed]);
        assert!(store.dispatches.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn claims_and_hands_a_zoom_job_to_the_background_worker() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        let outcome = dispatcher
            .start(
                "workspace-a",
                ZoomRtmsStarted {
                    account_id: "account-a".into(),
                    meeting_uuid: "meeting-uuid".into(),
                    meeting_id: "123".into(),
                    stream_id: "stream-id".into(),
                    signaling_url: format!("wss://{address}/signaling").parse().unwrap(),
                    event_timestamp_ms: 1,
                },
            )
            .await
            .unwrap();

        assert_eq!(outcome.job_id, "job-a");
        assert!(outcome.started);
        tokio::time::timeout(Duration::from_secs(5), store.terminal.notified())
            .await
            .expect("worker did not persist its terminal state");
        let events = store.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0].payload,
            CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Launching
        ));
        assert!(matches!(
            &events[1].payload,
            CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Failed
        ));
        assert_eq!(events[0].sequence, 0);
        assert_eq!(events[1].sequence, 1);
    }

    #[tokio::test]
    async fn applies_a_signed_stop_that_arrives_before_start_dispatch_finishes() {
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        let started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "meeting-uuid".into(),
            meeting_id: "123".into(),
            stream_id: "stream-id".into(),
            signaling_url: "wss://127.0.0.1:1/signaling".parse().unwrap(),
            event_timestamp_ms: 1,
        };
        store
            .save_capture_dispatch(&capture_dispatch("workspace-a", "job-a", &started).unwrap())
            .await
            .unwrap();
        dispatcher
            .stop(
                &ZoomRtmsTerminal {
                    meeting_uuid: "meeting-uuid".into(),
                    stream_id: "stream-id".into(),
                    event_timestamp_ms: 1,
                },
                ZoomStopReason::Stopped,
            )
            .await
            .unwrap();
        dispatcher.start("workspace-a", started).await.unwrap();

        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("worker did not persist the early stop");
        let events = store.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0].payload,
            CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Canceled
        ));
    }

    #[tokio::test]
    async fn a_terminal_stop_wins_over_an_interruption_in_either_order() {
        let stopped_then_interrupted = terminal_state_after_pending_stops(&[
            ZoomStopReason::Stopped,
            ZoomStopReason::Interrupted,
        ])
        .await;
        let interrupted_then_stopped = terminal_state_after_pending_stops(&[
            ZoomStopReason::Interrupted,
            ZoomStopReason::Stopped,
        ])
        .await;

        assert_eq!(stopped_then_interrupted, BotState::Canceled);
        assert_eq!(interrupted_then_stopped, BotState::Canceled);
    }

    #[tokio::test]
    async fn unknown_terminal_events_do_not_consume_pending_capacity() {
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        for index in 0..=MAX_PENDING_STOPS {
            dispatcher
                .stop(
                    &ZoomRtmsTerminal {
                        meeting_uuid: format!("unknown-meeting-{index}"),
                        stream_id: format!("unknown-stream-{index}"),
                        event_timestamp_ms: 1,
                    },
                    ZoomStopReason::Stopped,
                )
                .await
                .unwrap();
        }
        assert!(dispatcher.registry.lock().await.pending_stops.is_empty());

        let started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "meeting-uuid".into(),
            meeting_id: "123".into(),
            stream_id: "stream-id".into(),
            signaling_url: "wss://127.0.0.1:1/signaling".parse().unwrap(),
            event_timestamp_ms: 1,
        };
        store
            .save_capture_dispatch(&capture_dispatch("workspace-a", "job-a", &started).unwrap())
            .await
            .unwrap();
        dispatcher
            .stop(
                &ZoomRtmsTerminal {
                    meeting_uuid: started.meeting_uuid.clone(),
                    stream_id: started.stream_id.clone(),
                    event_timestamp_ms: 2,
                },
                ZoomStopReason::Stopped,
            )
            .await
            .unwrap();
        assert_eq!(dispatcher.registry.lock().await.pending_stops.len(), 1);
    }

    #[tokio::test]
    async fn interruption_keeps_the_capture_non_terminal_until_a_stop_arrives() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = accepted_tx.send(());
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(stream);
        });
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        let terminal = ZoomRtmsTerminal {
            meeting_uuid: "meeting-uuid".into(),
            stream_id: "stream-id".into(),
            event_timestamp_ms: 2,
        };
        dispatcher
            .start(
                "workspace-a",
                ZoomRtmsStarted {
                    account_id: "account-a".into(),
                    meeting_uuid: terminal.meeting_uuid.clone(),
                    meeting_id: "123".into(),
                    stream_id: terminal.stream_id.clone(),
                    signaling_url: format!("wss://{address}/signaling").parse().unwrap(),
                    event_timestamp_ms: 1,
                },
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), accepted_rx)
            .await
            .expect("worker did not begin connecting")
            .unwrap();

        dispatcher
            .stop(&terminal, ZoomStopReason::Interrupted)
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!store.dispatches.lock().unwrap().is_empty());
        assert!(store.events.lock().unwrap().iter().all(|event| {
            !matches!(
                &event.payload,
                CaptureEventPayload::Lifecycle(transition) if transition.to.is_terminal()
            )
        }));

        dispatcher
            .stop(&terminal, ZoomStopReason::Stopped)
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("worker did not persist the final stop");
        server.abort();
        let states = store
            .events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match &event.payload {
                CaptureEventPayload::Lifecycle(transition) => Some(transition.to),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![BotState::Launching, BotState::Stopping, BotState::Completed]
        );
    }

    #[tokio::test]
    async fn repeated_start_refreshes_the_dispatch_and_reconnects_immediately() {
        let first_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let first_address = first_listener.local_addr().unwrap();
        let second_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let second_address = second_listener.local_addr().unwrap();
        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        let first_server = tokio::spawn(async move {
            let (stream, _) = first_listener.accept().await.unwrap();
            let _ = first_tx.send(());
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(stream);
        });
        let (second_tx, second_rx) = tokio::sync::oneshot::channel();
        let second_server = tokio::spawn(async move {
            let (stream, _) = second_listener.accept().await.unwrap();
            let _ = second_tx.send(());
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(stream);
        });
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        let mut started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "meeting-uuid".into(),
            meeting_id: "123".into(),
            stream_id: "stream-id".into(),
            signaling_url: format!("wss://{first_address}/signaling").parse().unwrap(),
            event_timestamp_ms: 1,
        };
        dispatcher
            .start("workspace-a", started.clone())
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), first_rx)
            .await
            .expect("worker did not use the initial RTMS server")
            .unwrap();

        started.signaling_url = format!("wss://{second_address}/signaling").parse().unwrap();
        started.event_timestamp_ms = 2;
        let outcome = dispatcher
            .start("workspace-a", started.clone())
            .await
            .unwrap();
        assert!(!outcome.started);
        tokio::time::timeout(Duration::from_secs(2), second_rx)
            .await
            .expect("worker did not reconnect to the refreshed RTMS server")
            .unwrap();
        let dispatch = store.dispatches.lock().unwrap().first().cloned().unwrap();
        assert_eq!(dispatch.payload, serde_json::to_value(&started).unwrap());

        dispatcher
            .stop(
                &ZoomRtmsTerminal {
                    meeting_uuid: started.meeting_uuid,
                    stream_id: started.stream_id,
                    event_timestamp_ms: 3,
                },
                ZoomStopReason::Stopped,
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("worker did not stop after reconnecting");
        first_server.abort();
        second_server.abort();
    }

    #[tokio::test]
    async fn restart_is_not_acknowledged_by_a_tearing_down_registry_task() {
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        let started = ZoomRtmsStarted {
            account_id: "account-a".into(),
            meeting_uuid: "meeting-uuid".into(),
            meeting_id: "123".into(),
            stream_id: "stream-id".into(),
            signaling_url: "wss://127.0.0.1:1/signaling".parse().unwrap(),
            event_timestamp_ms: 1,
        };
        let (commands, mut receiver) = mpsc::unbounded_channel();
        dispatcher.registry.lock().await.tasks.insert(
            started.stream_id.clone(),
            ZoomCaptureTask {
                workspace_id: "workspace-a".into(),
                job_id: "job-a".into(),
                meeting_uuid: started.meeting_uuid.clone(),
                commands,
            },
        );
        let teardown = tokio::spawn(async move {
            let command = receiver.recv().await.unwrap();
            assert!(matches!(command, ZoomCaptureCommand::Restart { .. }));
        });

        let outcome = dispatcher
            .start("workspace-a", started.clone())
            .await
            .unwrap();

        teardown.await.unwrap();
        assert!(outcome.started);
        assert_eq!(outcome.job_id, "job-a");
        dispatcher
            .stop(
                &ZoomRtmsTerminal {
                    meeting_uuid: started.meeting_uuid,
                    stream_id: started.stream_id,
                    event_timestamp_ms: 2,
                },
                ZoomStopReason::Stopped,
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("replacement worker did not receive the stop");
    }

    #[tokio::test]
    async fn store_failures_terminalize_the_job_instead_of_abandoning_it() {
        let store = Arc::new(TestStore::with_append_failures(
            zoom_checkpoint("123"),
            STORE_RETRY_ATTEMPTS,
        ));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        dispatcher
            .start(
                "workspace-a",
                ZoomRtmsStarted {
                    account_id: "account-a".into(),
                    meeting_uuid: "meeting-uuid".into(),
                    meeting_id: "123".into(),
                    stream_id: "stream-id".into(),
                    signaling_url: "wss://127.0.0.1:1/signaling".parse().unwrap(),
                    event_timestamp_ms: 1,
                },
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(5), store.terminal.notified())
            .await
            .expect("worker abandoned the job after a store failure");
        let events = store.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 0);
        assert!(matches!(
            &events[0].payload,
            CaptureEventPayload::Lifecycle(transition) if transition.to == BotState::Failed
        ));
    }

    #[tokio::test]
    async fn signed_stop_cancels_an_in_progress_connection() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (accepted_tx, accepted_rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let _ = accepted_tx.send(());
            tokio::time::sleep(Duration::from_secs(30)).await;
            drop(stream);
        });
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        dispatcher
            .start(
                "workspace-a",
                ZoomRtmsStarted {
                    account_id: "account-a".into(),
                    meeting_uuid: "meeting-uuid".into(),
                    meeting_id: "123".into(),
                    stream_id: "stream-id".into(),
                    signaling_url: format!("wss://{address}/signaling").parse().unwrap(),
                    event_timestamp_ms: 1,
                },
            )
            .await
            .unwrap();
        let dispatch = store.dispatches.lock().unwrap().first().cloned().unwrap();
        assert_eq!(dispatch.workspace_id, "workspace-a");
        assert_eq!(dispatch.job_id, "job-a");
        assert_eq!(dispatch.dispatch_id, "stream-id");
        tokio::time::timeout(Duration::from_secs(2), store.launching.notified())
            .await
            .expect("worker did not enter launching");
        tokio::time::timeout(Duration::from_secs(2), accepted_rx)
            .await
            .expect("worker did not begin connecting")
            .unwrap();
        dispatcher
            .stop(
                &ZoomRtmsTerminal {
                    meeting_uuid: "meeting-uuid".into(),
                    stream_id: "stream-id".into(),
                    event_timestamp_ms: 2,
                },
                ZoomStopReason::Stopped,
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("worker did not cancel its pending connection");
        assert!(store.dispatches.lock().unwrap().is_empty());
        server.abort();
        let states = store
            .events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match &event.payload {
                CaptureEventPayload::Lifecycle(transition) => Some(transition.to),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![BotState::Launching, BotState::Stopping, BotState::Completed]
        );
    }

    #[tokio::test]
    async fn signed_stop_wins_when_the_socket_closes_first() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let store = Arc::new(TestStore::new(zoom_checkpoint("123")));
        let dispatcher = ZoomCaptureDispatcher::new(
            store.clone(),
            ZoomRtmsCredentials::new("client-id", "client-secret").unwrap(),
        );
        dispatcher
            .start(
                "workspace-a",
                ZoomRtmsStarted {
                    account_id: "account-a".into(),
                    meeting_uuid: "meeting-uuid".into(),
                    meeting_id: "123".into(),
                    stream_id: "stream-id".into(),
                    signaling_url: format!("wss://{address}/signaling").parse().unwrap(),
                    event_timestamp_ms: 1,
                },
            )
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), store.launching.notified())
            .await
            .expect("worker did not enter launching");
        dispatcher
            .stop(
                &ZoomRtmsTerminal {
                    meeting_uuid: "meeting-uuid".into(),
                    stream_id: "stream-id".into(),
                    event_timestamp_ms: 2,
                },
                ZoomStopReason::Stopped,
            )
            .await
            .unwrap();

        tokio::time::timeout(Duration::from_secs(2), store.terminal.notified())
            .await
            .expect("worker did not persist the signed stop");
        let states = store
            .events
            .lock()
            .unwrap()
            .iter()
            .filter_map(|event| match &event.payload {
                CaptureEventPayload::Lifecycle(transition) => Some(transition.to),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            states,
            vec![BotState::Launching, BotState::Stopping, BotState::Completed]
        );
    }
}
