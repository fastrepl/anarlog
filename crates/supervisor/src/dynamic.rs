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
        generation: u64,
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
            Self::ScheduledRestart { spec, generation } => f
                .debug_struct("ScheduledRestart")
                .field("id", &spec.id)
                .field("generation", generation)
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
    // Incremented on every external SpawnChild/TerminateChild for an id, so
    // scheduled restarts carrying an older generation are dropped instead of
    // resurrecting a child that was terminated or replaced during backoff.
    child_generations: HashMap<String, u64>,
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

    fn generation_of(&self, child_id: &str) -> u64 {
        self.child_generations.get(child_id).copied().unwrap_or(0)
    }

    // A successful external SpawnChild or a TerminateChild starts a new
    // lifecycle for the id: stale scheduled restarts are dropped via the
    // generation, and the failure history is cleared so backoff cannot carry
    // over from the old lifecycle.
    fn begin_new_generation(&mut self, child_id: &str) {
        *self
            .child_generations
            .entry(child_id.to_string())
            .or_insert(0) += 1;
        self.child_failure_state.remove(child_id);
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
            child_generations: HashMap::new(),
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
                let result = handle_spawn_child(&spec, true, state, myself.clone()).await;
                // Only a successful spawn begins a new lifecycle: a failed
                // replacement must not cancel the previous lifecycle's
                // pending restart or discard its backoff history.
                if result.is_ok() {
                    state.begin_new_generation(&spec.id);
                }
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
            DynamicSupervisorMsg::ScheduledRestart { spec, generation } => {
                if generation != state.generation_of(&spec.id) {
                    return Ok(());
                }
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
    // Bump even when the child is not active: it may have failed and be
    // waiting in backoff, and termination must invalidate that restart.
    state.begin_new_generation(child_id);
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
    let generation = state.generation_of(&child_id);
    match delay {
        Some(d) => {
            let dur = ractor::concurrency::Duration::from_millis(d.as_millis() as u64);
            myself.send_after(dur, move || DynamicSupervisorMsg::ScheduledRestart {
                spec,
                generation,
            });
        }
        None => {
            myself.send_message(DynamicSupervisorMsg::ScheduledRestart { spec, generation })?;
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

    #[tokio::test(start_paused = true)]
    async fn terminate_during_backoff_does_not_resurrect() {
        let sup_name = unique_name("dyn_term_backoff_sup");
        let child_name = unique_name("dyn_term_backoff_child");
        let counter = Arc::new(AtomicU32::new(0));

        let mut spec = make_spec(
            &child_name,
            RestartPolicy::Permanent,
            ChildBehavior::DelayedFail { ms: 20 },
            counter.clone(),
        );
        spec.backoff_fn = Some(ChildBackoffFn::new(|_id, _count, _last, _reset| {
            Some(Duration::from_millis(200))
        }));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(5))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(sup_ref.clone(), spec)
            .await
            .expect("failed to spawn child");

        // Child fails at ~20ms; its restart is pending until ~220ms.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        DynamicSupervisor::terminate_child(sup_ref.clone(), child_name)
            .await
            .expect("failed to terminate child");

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            counter.load(Ordering::SeqCst),
            1,
            "terminated child was resurrected by a stale scheduled restart"
        );
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

    #[tokio::test(start_paused = true)]
    async fn replace_before_restart_keeps_replacement() {
        let sup_name = unique_name("dyn_replace_sup");
        let child_name = unique_name("dyn_replace_child");
        let fail_counter = Arc::new(AtomicU32::new(0));
        let healthy_counter = Arc::new(AtomicU32::new(0));

        let mut failing_spec = make_spec(
            &child_name,
            RestartPolicy::Permanent,
            ChildBehavior::DelayedFail { ms: 20 },
            fail_counter.clone(),
        );
        failing_spec.backoff_fn = Some(ChildBackoffFn::new(|_id, _count, _last, _reset| {
            Some(Duration::from_millis(200))
        }));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(5))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(sup_ref.clone(), failing_spec)
            .await
            .expect("failed to spawn failing child");

        // Replace the child under the same id while its restart is pending.
        tokio::time::sleep(Duration::from_millis(50)).await;
        let replacement = make_spec(
            &child_name,
            RestartPolicy::Permanent,
            ChildBehavior::Healthy,
            healthy_counter.clone(),
        );
        DynamicSupervisor::spawn_child(sup_ref.clone(), replacement)
            .await
            .expect("failed to spawn replacement child");

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            fail_counter.load(Ordering::SeqCst),
            1,
            "stale scheduled restart clobbered the deliberate replacement"
        );
        assert_eq!(healthy_counter.load(Ordering::SeqCst), 1);
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert_eq!(
            sup_ref
                .get_children()
                .iter()
                .filter(|c| c.get_status() == ActorStatus::Running)
                .count(),
            1
        );

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test(start_paused = true)]
    async fn failed_replacement_spawn_keeps_pending_restart() {
        let sup_name = unique_name("dyn_failed_replace_sup");
        let child_name = unique_name("dyn_failed_replace_child");
        let counter = Arc::new(AtomicU32::new(0));

        let mut spec = make_spec(
            &child_name,
            RestartPolicy::Permanent,
            ChildBehavior::DelayedFail { ms: 20 },
            counter.clone(),
        );
        spec.backoff_fn = Some(ChildBackoffFn::new(|_id, _count, _last, _reset| {
            Some(Duration::from_millis(200))
        }));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(10))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(sup_ref.clone(), spec.clone())
            .await
            .expect("failed to spawn child");

        // Child fails at ~20ms; its restart is pending until ~220ms.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        let mut broken_spec = spec;
        broken_spec.spawn_fn = DynSpawnFn::new(|_sup_cell, _child_id| async {
            Err(ractor::SpawnErr::StartupFailed(
                "injected spawn failure".into(),
            ))
        });
        DynamicSupervisor::spawn_child(sup_ref.clone(), broken_spec)
            .await
            .expect_err("broken replacement spawn should fail");

        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            counter.load(Ordering::SeqCst) >= 2,
            "failed replacement spawn cancelled the pending scheduled restart"
        );
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test(start_paused = true)]
    async fn respawn_after_terminate_starts_with_fresh_backoff_history() {
        let sup_name = unique_name("dyn_fresh_backoff_sup");
        let child_name = unique_name("dyn_fresh_backoff_child");
        let counter = Arc::new(AtomicU32::new(0));
        let observed_counts = Arc::new(std::sync::Mutex::new(Vec::new()));

        let spec = |counts: Arc<std::sync::Mutex<Vec<usize>>>, counter: Arc<AtomicU32>| {
            let mut spec = make_spec(
                &child_name,
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 20 },
                counter,
            );
            spec.backoff_fn = Some(ChildBackoffFn::new(move |_id, count, _last, _reset| {
                counts.lock().unwrap().push(count);
                Some(Duration::from_millis(200))
            }));
            spec
        };

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(10))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            spec(observed_counts.clone(), counter.clone()),
        )
        .await
        .expect("failed to spawn child");

        // Let the child fail twice so its per-child failure count escalates.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(observed_counts.lock().unwrap().len() >= 2);

        DynamicSupervisor::terminate_child(sup_ref.clone(), child_name.clone())
            .await
            .expect("failed to terminate child");
        observed_counts.lock().unwrap().clear();

        DynamicSupervisor::spawn_child(
            sup_ref.clone(),
            spec(observed_counts.clone(), counter.clone()),
        )
        .await
        .expect("failed to respawn child");
        tokio::time::sleep(Duration::from_millis(50)).await;

        // The new lifecycle's first failure must not inherit the old count.
        assert_eq!(
            observed_counts.lock().unwrap().first().copied(),
            Some(1),
            "backoff saw stale failure history from the previous lifecycle"
        );

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test(start_paused = true)]
    async fn repeated_failure_still_restarts_with_generations() {
        let sup_name = unique_name("dyn_repeat_fail_sup");
        let child_name = unique_name("dyn_repeat_fail_child");
        let counter = Arc::new(AtomicU32::new(0));

        let mut spec = make_spec(
            &child_name,
            RestartPolicy::Permanent,
            ChildBehavior::DelayedFail { ms: 20 },
            counter.clone(),
        );
        spec.backoff_fn = Some(ChildBackoffFn::new(|_id, _count, _last, _reset| {
            Some(Duration::from_millis(50))
        }));

        let (sup_ref, sup_handle) = DynamicSupervisor::spawn(sup_name, options(10))
            .await
            .expect("failed to spawn dynamic supervisor");
        DynamicSupervisor::spawn_child(sup_ref.clone(), spec)
            .await
            .expect("failed to spawn child");

        tokio::time::sleep(Duration::from_millis(320)).await;
        assert!(
            counter.load(Ordering::SeqCst) >= 3,
            "restarts within one generation should keep working, got {}",
            counter.load(Ordering::SeqCst)
        );
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);

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
