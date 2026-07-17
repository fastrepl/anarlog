use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ractor::{
    Actor, ActorCell, ActorId, ActorProcessingErr, ActorRef, RpcReplyPort, SpawnErr,
    SupervisionEvent, concurrency::JoinHandle,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, thiserror::Error)]
pub enum SupervisorError {
    #[error("Child '{child_id}' not found in specs")]
    ChildNotFound { child_id: String },

    #[error("Child '{pid}' does not have a name set")]
    ChildNameNotSet { pid: ActorId },

    #[error("Max children exceeded")]
    MaxChildrenExceeded,

    #[error("Meltdown: {reason}")]
    Meltdown { reason: String },
}

pub type DynSpawnFuture = Pin<Box<dyn Future<Output = Result<ActorCell, SpawnErr>> + Send>>;

#[derive(Clone)]
pub struct DynSpawnFn(Arc<dyn Fn(ActorCell, String) -> DynSpawnFuture + Send + Sync>);

impl DynSpawnFn {
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(ActorCell, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ActorCell, SpawnErr>> + Send + 'static,
    {
        Self(Arc::new(move |cell, id| Box::pin(f(cell, id))))
    }

    pub async fn call(&self, sup: ActorCell, id: String) -> Result<ActorCell, SpawnErr> {
        (self.0)(sup, id).await
    }
}

type BackoffFn = dyn Fn(&str, usize, Instant, Option<Duration>) -> Option<Duration> + Send + Sync;

#[derive(Clone)]
pub struct ChildBackoffFn(Arc<BackoffFn>);

impl ChildBackoffFn {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(&str, usize, Instant, Option<Duration>) -> Option<Duration> + Send + Sync + 'static,
    {
        Self(Arc::new(f))
    }

    fn call(
        &self,
        child_id: &str,
        restart_count: usize,
        last_fail: Instant,
        reset_after: Option<Duration>,
    ) -> Option<Duration> {
        (self.0)(child_id, restart_count, last_fail, reset_after)
    }
}

#[derive(Clone)]
pub struct DynChildSpec {
    pub id: String,
    pub restart: crate::RestartPolicy,
    pub spawn_fn: DynSpawnFn,
    pub backoff_fn: Option<ChildBackoffFn>,
    pub reset_after: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct DynamicSupervisorOptions {
    pub max_children: Option<usize>,
    pub max_restarts: usize,
    pub max_window: Duration,
    pub reset_after: Option<Duration>,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

pub enum DynamicSupervisorMsg {
    SpawnChild {
        spec: DynChildSpec,
        reply: Option<RpcReplyPort<Result<(), ActorProcessingErr>>>,
    },
    TerminateChild {
        child_id: String,
        reply: Option<RpcReplyPort<()>>,
    },
    ScheduledRestart {
        spec: DynChildSpec,
    },
}

impl std::fmt::Debug for DynamicSupervisorMsg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnChild { spec, .. } => {
                f.debug_struct("SpawnChild").field("id", &spec.id).finish()
            }
            Self::TerminateChild { child_id, .. } => f
                .debug_struct("TerminateChild")
                .field("child_id", child_id)
                .finish(),
            Self::ScheduledRestart { spec } => f
                .debug_struct("ScheduledRestart")
                .field("id", &spec.id)
                .finish(),
        }
    }
}

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct ActiveChild {
    spec: DynChildSpec,
    cell: ActorCell,
}

struct ChildFailureState {
    restart_count: usize,
    last_fail: Instant,
}

struct RestartLogEntry {
    _child_id: String,
    timestamp: Instant,
}

pub struct DynamicSupervisorState {
    options: DynamicSupervisorOptions,
    active_children: HashMap<String, ActiveChild>,
    child_failure_state: HashMap<String, ChildFailureState>,
    restart_log: Vec<RestartLogEntry>,
}

// ---------------------------------------------------------------------------
// Actor
// ---------------------------------------------------------------------------

pub struct DynamicSupervisor;

impl DynamicSupervisor {
    pub async fn spawn(
        name: String,
        options: DynamicSupervisorOptions,
    ) -> Result<(ActorRef<DynamicSupervisorMsg>, JoinHandle<()>), SpawnErr> {
        Actor::spawn(Some(name), DynamicSupervisor, options).await
    }

    pub async fn spawn_linked<T: Actor>(
        name: impl Into<String>,
        handler: T,
        args: T::Arguments,
        supervisor: ActorCell,
    ) -> Result<(ActorRef<T::Msg>, JoinHandle<()>), SpawnErr> {
        Actor::spawn_linked(Some(name.into()), handler, args, supervisor).await
    }

    pub async fn spawn_child(
        sup_ref: ActorRef<DynamicSupervisorMsg>,
        spec: DynChildSpec,
    ) -> Result<(), ActorProcessingErr> {
        ractor::call!(sup_ref, |reply| {
            DynamicSupervisorMsg::SpawnChild {
                spec,
                reply: Some(reply),
            }
        })?
    }

    pub async fn terminate_child(
        sup_ref: ActorRef<DynamicSupervisorMsg>,
        child_id: String,
    ) -> Result<(), ActorProcessingErr> {
        ractor::call!(sup_ref, |reply| {
            DynamicSupervisorMsg::TerminateChild {
                child_id,
                reply: Some(reply),
            }
        })?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Meltdown tracking
// ---------------------------------------------------------------------------

impl DynamicSupervisorState {
    fn track_global_restart(&mut self, child_id: &str) -> Result<(), ActorProcessingErr> {
        let now = Instant::now();

        if let Some(reset_after) = self.options.reset_after
            && let Some(latest) = self.restart_log.last()
            && now.duration_since(latest.timestamp) >= reset_after
        {
            self.restart_log.clear();
        }

        self.restart_log.push(RestartLogEntry {
            _child_id: child_id.to_string(),
            timestamp: now,
        });

        self.restart_log
            .retain(|e| now.duration_since(e.timestamp) < self.options.max_window);

        if self.restart_log.len() > self.options.max_restarts {
            Err(SupervisorError::Meltdown {
                reason: "max_restarts exceeded".to_string(),
            }
            .into())
        } else {
            Ok(())
        }
    }

    fn prepare_child_failure(&mut self, spec: &DynChildSpec) {
        let now = Instant::now();
        let entry = self
            .child_failure_state
            .entry(spec.id.clone())
            .or_insert(ChildFailureState {
                restart_count: 0,
                last_fail: now,
            });

        if let Some(threshold) = spec.reset_after
            && now.duration_since(entry.last_fail) >= threshold
        {
            entry.restart_count = 0;
        }

        entry.restart_count += 1;
        entry.last_fail = now;
    }
}

// ---------------------------------------------------------------------------
// Actor implementation
// ---------------------------------------------------------------------------

#[ractor::async_trait]
impl Actor for DynamicSupervisor {
    type Msg = DynamicSupervisorMsg;
    type State = DynamicSupervisorState;
    type Arguments = DynamicSupervisorOptions;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        options: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        Ok(DynamicSupervisorState {
            options,
            active_children: HashMap::new(),
            child_failure_state: HashMap::new(),
            restart_log: Vec::new(),
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            DynamicSupervisorMsg::SpawnChild { spec, reply } => {
                let result =
                    handle_spawn_child(&spec, reply.is_some(), state, myself.clone()).await;
                if let Some(reply) = reply {
                    reply.send(result)?;
                    Ok(())
                } else {
                    result
                }
            }
            DynamicSupervisorMsg::TerminateChild { child_id, reply } => {
                handle_terminate_child(&child_id, state, &myself);
                if let Some(reply) = reply {
                    reply.send(())?;
                }
                Ok(())
            }
            DynamicSupervisorMsg::ScheduledRestart { spec } => {
                handle_spawn_child(&spec, false, state, myself).await
            }
        }
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        evt: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match evt {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {}

            SupervisionEvent::ActorTerminated(cell, _, reason) => {
                handle_child_restart(cell, false, state, &myself, reason.as_deref())?;
            }

            SupervisionEvent::ActorFailed(cell, err) => {
                let reason = format!("{:?}", err);
                handle_child_restart(cell, true, state, &myself, Some(&reason))?;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn handle_spawn_child(
    spec: &DynChildSpec,
    first_start: bool,
    state: &mut DynamicSupervisorState,
    myself: ActorRef<DynamicSupervisorMsg>,
) -> Result<(), ActorProcessingErr> {
    if !first_start {
        state.track_global_restart(&spec.id)?;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    if let Some(max) = state.options.max_children
        && state.active_children.len() >= max
    {
        return Err(SupervisorError::MaxChildrenExceeded.into());
    }

    let result = spec.spawn_fn.call(myself.get_cell(), spec.id.clone()).await;

    match result {
        Ok(child_cell) => {
            state.active_children.insert(
                spec.id.clone(),
                ActiveChild {
                    spec: spec.clone(),
                    cell: child_cell,
                },
            );
            state
                .child_failure_state
                .entry(spec.id.clone())
                .or_insert(ChildFailureState {
                    restart_count: 0,
                    last_fail: Instant::now(),
                });
            Ok(())
        }
        Err(e) => Err(SupervisorError::Meltdown {
            reason: format!("spawn failed for '{}': {}", spec.id, e),
        }
        .into()),
    }
}

fn handle_terminate_child(
    child_id: &str,
    state: &mut DynamicSupervisorState,
    myself: &ActorRef<DynamicSupervisorMsg>,
) {
    if let Some(child) = state.active_children.remove(child_id) {
        child.cell.unlink(myself.get_cell());
        child.cell.kill();
    }
}

fn handle_child_restart(
    cell: ActorCell,
    abnormal: bool,
    state: &mut DynamicSupervisorState,
    myself: &ActorRef<DynamicSupervisorMsg>,
    _reason: Option<&str>,
) -> Result<(), ActorProcessingErr> {
    let child_id = cell
        .get_name()
        .ok_or(SupervisorError::ChildNameNotSet { pid: cell.get_id() })?;

    let child = match state.active_children.remove(&child_id) {
        Some(c) => c,
        None => return Ok(()),
    };

    let should_restart = match child.spec.restart {
        crate::RestartPolicy::Permanent => true,
        crate::RestartPolicy::Transient => abnormal,
        crate::RestartPolicy::Temporary => false,
    };

    if !should_restart {
        return Ok(());
    }

    state.prepare_child_failure(&child.spec);

    let delay = child.spec.backoff_fn.as_ref().and_then(|bf| {
        let fs = state.child_failure_state.get(&child.spec.id);
        let (count, last_fail) = fs
            .map(|f| (f.restart_count, f.last_fail))
            .unwrap_or((0, Instant::now()));
        bf.call(&child.spec.id, count, last_fail, child.spec.reset_after)
    });

    let spec = child.spec.clone();
    match delay {
        Some(d) => {
            let dur = ractor::concurrency::Duration::from_millis(d.as_millis() as u64);
            myself.send_after(dur, move || DynamicSupervisorMsg::ScheduledRestart { spec });
        }
        None => {
            myself.send_message(DynamicSupervisorMsg::ScheduledRestart { spec })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RestartPolicy;
    use ractor::{ActorRef, ActorStatus};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn unique_name(prefix: &str) -> String {
        format!(
            "{prefix}_{}_{}",
            std::process::id(),
            TEST_SEQ.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[derive(Clone)]
    enum ChildBehavior {
        Healthy,
        DelayedFail { ms: u64 },
        DelayedNormal { ms: u64 },
    }

    struct TestChild {
        counter: Arc<AtomicU32>,
    }

    #[ractor::async_trait]
    impl Actor for TestChild {
        type Msg = ();
        type State = ChildBehavior;
        type Arguments = ChildBehavior;

        async fn pre_start(
            &self,
            myself: ActorRef<Self::Msg>,
            behavior: Self::Arguments,
        ) -> Result<Self::State, ActorProcessingErr> {
            self.counter.fetch_add(1, Ordering::SeqCst);

            match behavior {
                ChildBehavior::DelayedFail { ms } | ChildBehavior::DelayedNormal { ms } => {
                    myself.send_after(Duration::from_millis(ms), || ());
                }
                ChildBehavior::Healthy => {}
            }

            Ok(behavior)
        }

        async fn handle(
            &self,
            myself: ActorRef<Self::Msg>,
            _msg: Self::Msg,
            state: &mut Self::State,
        ) -> Result<(), ActorProcessingErr> {
            match state {
                ChildBehavior::DelayedFail { .. } => panic!("delayed_fail"),
                ChildBehavior::DelayedNormal { .. } => myself.stop(None),
                ChildBehavior::Healthy => {}
            }
            Ok(())
        }
    }

    fn make_spec(
        id: &str,
        restart: RestartPolicy,
        behavior: ChildBehavior,
        counter: Arc<AtomicU32>,
    ) -> DynChildSpec {
        let id = id.to_string();
        DynChildSpec {
            id: id.clone(),
            restart,
            spawn_fn: DynSpawnFn::new(move |sup_cell, child_id| {
                let behavior = behavior.clone();
                let counter = counter.clone();
                async move {
                    let (child_ref, _join) = DynamicSupervisor::spawn_linked(
                        child_id,
                        TestChild { counter },
                        behavior,
                        sup_cell,
                    )
                    .await?;
                    Ok(child_ref.get_cell())
                }
            }),
            backoff_fn: None,
            reset_after: None,
        }
    }

    fn options(max_restarts: usize) -> DynamicSupervisorOptions {
        DynamicSupervisorOptions {
            max_children: None,
            max_restarts,
            max_window: Duration::from_secs(5),
            reset_after: None,
        }
    }

    #[tokio::test]
    async fn transient_child_no_restart_on_normal_exit() {
        let sup_name = unique_name("dyn_transient_normal_sup");
        let child_name = unique_name("dyn_transient_normal_child");
        let counter = Arc::new(AtomicU32::new(0));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(5))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name,
                RestartPolicy::Transient,
                ChildBehavior::DelayedNormal { ms: 50 },
                counter.clone(),
            ),
        )
        .await
        .expect("failed to spawn child");

        tokio::time::sleep(Duration::from_millis(180)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(
            !sup_ref
                .get_children()
                .iter()
                .any(|c| c.get_status() == ActorStatus::Running)
        );

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn temporary_child_never_restarts_on_failure() {
        let sup_name = unique_name("dyn_temporary_sup");
        let child_name = unique_name("dyn_temporary_child");
        let counter = Arc::new(AtomicU32::new(0));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(5))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name,
                RestartPolicy::Temporary,
                ChildBehavior::DelayedFail { ms: 50 },
                counter.clone(),
            ),
        )
        .await
        .expect("failed to spawn child");

        tokio::time::sleep(Duration::from_millis(180)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert!(
            !sup_ref
                .get_children()
                .iter()
                .any(|c| c.get_status() == ActorStatus::Running)
        );

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn permanent_child_triggers_meltdown_when_budget_exceeded() {
        let sup_name = unique_name("dyn_meltdown_sup");
        let child_name = unique_name("dyn_meltdown_child");
        let counter = Arc::new(AtomicU32::new(0));

        let mut opts = options(1);
        opts.max_window = Duration::from_secs(2);

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, opts)
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name,
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 40 },
                counter.clone(),
            ),
        )
        .await
        .expect("failed to spawn child");

        let _ = sup_handle.await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Stopped);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn reset_after_allows_restarts_across_quiet_periods() {
        let sup_name = unique_name("dyn_reset_after_sup");
        let child_name = unique_name("dyn_reset_after_child");
        let counter = Arc::new(AtomicU32::new(0));

        let mut opts = options(1);
        opts.max_window = Duration::from_secs(10);
        opts.reset_after = Some(Duration::from_millis(80));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, opts)
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name,
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 140 },
                counter.clone(),
            ),
        )
        .await
        .expect("failed to spawn child");

        tokio::time::sleep(Duration::from_millis(520)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert!(counter.load(Ordering::SeqCst) >= 3);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn max_children_is_enforced() {
        let sup_name = unique_name("dyn_max_children_sup");
        let child_name_1 = unique_name("dyn_max_children_child1");
        let child_name_2 = unique_name("dyn_max_children_child2");
        let counter = Arc::new(AtomicU32::new(0));

        let mut opts = options(5);
        opts.max_children = Some(1);

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, opts)
            .await
            .expect("failed to spawn dynamic supervisor");

        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name_1,
                RestartPolicy::Permanent,
                ChildBehavior::Healthy,
                counter.clone(),
            ),
        )
        .await
        .expect("first child should spawn");

        let second = DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name_2,
                RestartPolicy::Permanent,
                ChildBehavior::Healthy,
                counter.clone(),
            ),
        )
        .await;

        assert!(second.is_err());
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn terminate_child_does_not_restart() {
        let sup_name = unique_name("dyn_terminate_sup");
        let child_name = unique_name("dyn_terminate_child");
        let counter = Arc::new(AtomicU32::new(0));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(5))
            .await
            .expect("failed to spawn dynamic supervisor");

        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            make_spec(
                &child_name,
                RestartPolicy::Permanent,
                ChildBehavior::Healthy,
                counter.clone(),
            ),
        )
        .await
        .expect("failed to spawn child");

        tokio::time::sleep(Duration::from_millis(40)).await;
        DynamicSupervisor::terminate_child(sup_ref.clone(), child_name)
            .await
            .expect("failed to terminate child");
        tokio::time::sleep(Duration::from_millis(120)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert!(
            !sup_ref
                .get_children()
                .iter()
                .any(|c| c.get_status() == ActorStatus::Running)
        );

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn backoff_delays_second_restart_attempt() {
        let sup_name = unique_name("dyn_backoff_sup");
        let child_name = unique_name("dyn_backoff_child");
        let counter = Arc::new(AtomicU32::new(0));

        let mut spec = make_spec(
            &child_name,
            RestartPolicy::Permanent,
            ChildBehavior::DelayedFail { ms: 10 },
            counter.clone(),
        );
        spec.backoff_fn = Some(ChildBackoffFn::new(
            |_id, restart_count, _last, _child_reset| {
                if restart_count <= 1 {
                    None
                } else {
                    Some(Duration::from_millis(220))
                }
            },
        ));

        let mut opts = options(1);
        opts.max_window = Duration::from_secs(5);

        let start = std::time::Instant::now();
        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, opts)
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(sup_ref.clone(), spec)
            .await
            .expect("failed to spawn child");

        let _ = sup_handle.await;
        let elapsed = start.elapsed();
        assert_eq!(sup_ref.get_status(), ActorStatus::Stopped);
        assert!(
            elapsed >= Duration::from_millis(200),
            "expected delayed restart, got {elapsed:?}"
        );
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }
}
