use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use ractor::{Actor, ActorCell, ActorProcessingErr, ActorRef, SpawnErr, SupervisionEvent};

use crate::restart::{RestartBudget, RestartTracker};
use crate::retry::{RetryStrategy, spawn_with_retry};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    Permanent,
    Transient,
    Temporary,
}

pub type SpawnFuture = Pin<Box<dyn Future<Output = Result<ActorCell, SpawnErr>> + Send>>;

#[derive(Clone)]
pub struct SpawnFn(Arc<dyn Fn(ActorCell) -> SpawnFuture + Send + Sync>);

impl SpawnFn {
    pub fn new<F, Fut>(f: F) -> Self
    where
        F: Fn(ActorCell) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ActorCell, SpawnErr>> + Send + 'static,
    {
        Self(Arc::new(move |cell| Box::pin(f(cell))))
    }

    pub async fn call(&self, supervisor_cell: ActorCell) -> Result<ActorCell, SpawnErr> {
        (self.0)(supervisor_cell).await
    }
}

pub struct ChildSpec {
    pub id: String,
    pub restart_policy: RestartPolicy,
    pub spawn_fn: SpawnFn,
}

pub struct SupervisorConfig {
    pub children: Vec<ChildSpec>,
    pub restart_budget: RestartBudget,
    pub retry_strategy: RetryStrategy,
}

struct ChildEntry {
    id: String,
    cell: Option<ActorCell>,
    policy: RestartPolicy,
    spawn_fn: SpawnFn,
    tracker: RestartTracker,
}

pub struct SupervisorState {
    children: Vec<ChildEntry>,
    budget: RestartBudget,
    retry_strategy: RetryStrategy,
    shutting_down: bool,
}

pub struct Supervisor;

#[derive(Debug)]
pub enum SupervisorMsg {
    Shutdown,
}

impl SupervisorState {
    fn find_child_index(&self, cell: &ActorCell) -> Option<usize> {
        self.children
            .iter()
            .position(|e| e.cell.as_ref().is_some_and(|c| c.get_id() == cell.get_id()))
    }

    fn stop_all_children(&mut self) {
        for entry in &mut self.children {
            if let Some(cell) = entry.cell.take() {
                cell.stop(Some("supervisor_shutdown".to_string()));
            }
        }
    }
}

fn should_restart(policy: RestartPolicy, abnormal: bool) -> bool {
    match policy {
        RestartPolicy::Permanent => true,
        RestartPolicy::Transient => abnormal,
        RestartPolicy::Temporary => false,
    }
}

async fn handle_child_exit(
    myself: &ActorRef<SupervisorMsg>,
    state: &mut SupervisorState,
    idx: usize,
    abnormal: bool,
) {
    state.children[idx].cell = None;

    if !should_restart(state.children[idx].policy, abnormal) {
        return;
    }

    if !state.children[idx].tracker.record_restart(&state.budget) {
        tracing::error!(child = %state.children[idx].id, "restart_limit_exceeded");
        state.shutting_down = true;
        state.stop_all_children();
        myself.stop(Some("meltdown".to_string()));
        return;
    }

    let spawn_fn = state.children[idx].spawn_fn.clone();
    let sup_cell = myself.get_cell();
    let retry = state.retry_strategy.clone();

    let new_cell = spawn_with_retry(&retry, || {
        let sup = sup_cell.clone();
        let sf = spawn_fn.clone();
        async move { sf.call(sup).await }
    })
    .await;

    match new_cell {
        Some(cell) => {
            state.children[idx].cell = Some(cell);
        }
        None => {
            tracing::error!(child = %state.children[idx].id, "spawn_retry_exhausted");
            state.shutting_down = true;
            state.stop_all_children();
            myself.stop(Some("spawn_retry_exhausted".to_string()));
        }
    }
}

#[ractor::async_trait]
impl Actor for Supervisor {
    type Msg = SupervisorMsg;
    type State = SupervisorState;
    type Arguments = SupervisorConfig;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        config: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let mut children = Vec::new();

        for spec in config.children {
            let cell =
                spec.spawn_fn
                    .call(myself.get_cell())
                    .await
                    .map_err(|e| -> ActorProcessingErr {
                        format!("failed to spawn child '{}': {}", spec.id, e).into()
                    })?;

            children.push(ChildEntry {
                id: spec.id,
                cell: Some(cell),
                policy: spec.restart_policy,
                spawn_fn: spec.spawn_fn,
                tracker: RestartTracker::new(),
            });
        }

        Ok(SupervisorState {
            children,
            budget: config.restart_budget,
            retry_strategy: config.retry_strategy,
            shutting_down: false,
        })
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            SupervisorMsg::Shutdown => {
                state.shutting_down = true;
                state.stop_all_children();
                myself.stop(None);
            }
        }
        Ok(())
    }

    async fn handle_supervisor_evt(
        &self,
        myself: ActorRef<Self::Msg>,
        message: SupervisionEvent,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        for child in &mut state.children {
            child.tracker.maybe_reset(&state.budget);
        }

        if state.shutting_down {
            return Ok(());
        }

        match message {
            SupervisionEvent::ActorStarted(_) | SupervisionEvent::ProcessGroupChanged(_) => {}

            SupervisionEvent::ActorTerminated(cell, _, _reason) => {
                if let Some(idx) = state.find_child_index(&cell) {
                    handle_child_exit(&myself, state, idx, false).await;
                }
            }

            SupervisionEvent::ActorFailed(cell, _error) => {
                if let Some(idx) = state.find_child_index(&cell) {
                    handle_child_exit(&myself, state, idx, true).await;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ractor::{Actor, ActorRef, ActorStatus};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::Duration;

    // ---- Test child actor with configurable behaviors ----

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

            match &behavior {
                ChildBehavior::Healthy => {}
                ChildBehavior::DelayedFail { ms } => {
                    myself.send_after(Duration::from_millis(*ms), || ());
                }
                ChildBehavior::DelayedNormal { ms } => {
                    myself.send_after(Duration::from_millis(*ms), || ());
                }
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
                _ => {}
            }
            Ok(())
        }
    }

    // ---- Helpers ----

    fn make_child_spec(
        name: &str,
        policy: RestartPolicy,
        behavior: ChildBehavior,
        counter: Arc<AtomicU32>,
    ) -> ChildSpec {
        let name = name.to_string();
        ChildSpec {
            id: name.clone(),
            restart_policy: policy,
            spawn_fn: SpawnFn::new(move |sup_cell| {
                let behavior = behavior.clone();
                let counter = counter.clone();
                let name = name.clone();
                async move {
                    let (actor_ref, _) =
                        Actor::spawn_linked(Some(name), TestChild { counter }, behavior, sup_cell)
                            .await?;
                    Ok(actor_ref.get_cell())
                }
            }),
        }
    }

    fn test_budget(max_restarts: u32) -> RestartBudget {
        RestartBudget {
            max_restarts,
            max_window: Duration::from_secs(10),
            reset_after: None,
        }
    }

    fn fast_retry() -> RetryStrategy {
        RetryStrategy {
            max_attempts: 3,
            base_delay: Duration::from_millis(20),
        }
    }

    // ---- Tests ----

    #[tokio::test]
    async fn permanent_child_restarts_on_failure() {
        let counter = Arc::new(AtomicU32::new(0));
        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "perm_restart_child",
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 100 },
                counter.clone(),
            )],
            restart_budget: test_budget(1),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_perm_restart".to_string()), Supervisor, config)
                .await
                .unwrap();

        // Child fails once -> restarted -> fails again -> meltdown
        let _ = sup_handle.await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Stopped);
        // initial + 1 restart = 2 spawns
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn transient_no_restart_on_normal_exit() {
        let counter = Arc::new(AtomicU32::new(0));
        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "trans_normal_child",
                RestartPolicy::Transient,
                ChildBehavior::DelayedNormal { ms: 100 },
                counter.clone(),
            )],
            restart_budget: test_budget(5),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_trans_normal".to_string()), Supervisor, config)
                .await
                .unwrap();

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn transient_restarts_on_failure() {
        let counter = Arc::new(AtomicU32::new(0));
        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "trans_fail_child",
                RestartPolicy::Transient,
                ChildBehavior::DelayedFail { ms: 100 },
                counter.clone(),
            )],
            restart_budget: test_budget(1),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_trans_fail".to_string()), Supervisor, config)
                .await
                .unwrap();

        let _ = sup_handle.await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Stopped);
        assert_eq!(counter.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn temporary_never_restarts() {
        let counter = Arc::new(AtomicU32::new(0));
        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "temp_child",
                RestartPolicy::Temporary,
                ChildBehavior::DelayedFail { ms: 100 },
                counter.clone(),
            )],
            restart_budget: test_budget(5),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_temp_never".to_string()), Supervisor, config)
                .await
                .unwrap();

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn meltdown_on_budget_exceeded() {
        let counter = Arc::new(AtomicU32::new(0));
        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "meltdown_child",
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 50 },
                counter.clone(),
            )],
            restart_budget: test_budget(2),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_meltdown".to_string()), Supervisor, config)
                .await
                .unwrap();

        let _ = sup_handle.await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Stopped);
        // initial + 2 restarts = 3 spawns, then meltdown on 3rd failure
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn reset_after_quiet_period() {
        let counter = Arc::new(AtomicU32::new(0));

        // Budget: 1 restart in 10s window, but reset_after 200ms
        let budget = RestartBudget {
            max_restarts: 1,
            max_window: Duration::from_secs(10),
            reset_after: Some(Duration::from_millis(200)),
        };

        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "reset_child",
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 300 },
                counter.clone(),
            )],
            restart_budget: budget,
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_reset_quiet".to_string()), Supervisor, config)
                .await
                .unwrap();

        // Child fails at ~300ms, restarted (count=1).
        // Fails again at ~600ms. reset_after=200ms < 300ms gap => counter resets => count=1 again, no meltdown.
        // Let it run through a few cycles.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert!(counter.load(Ordering::SeqCst) >= 3);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn shutdown_suppresses_restarts() {
        let counter = Arc::new(AtomicU32::new(0));
        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "shutdown_child",
                RestartPolicy::Permanent,
                ChildBehavior::Healthy,
                counter.clone(),
            )],
            restart_budget: test_budget(5),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) = Actor::spawn(
            Some("test_shutdown_suppress".to_string()),
            Supervisor,
            config,
        )
        .await
        .unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);

        let _ = sup_ref.cast(SupervisorMsg::Shutdown);
        let _ = sup_handle.await;

        assert_eq!(sup_ref.get_status(), ActorStatus::Stopped);
        // Only spawned once; shutdown should not trigger restarts
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn multiple_children_independent() {
        let healthy_counter = Arc::new(AtomicU32::new(0));
        let failing_counter = Arc::new(AtomicU32::new(0));

        let config = SupervisorConfig {
            children: vec![
                make_child_spec(
                    "multi_healthy",
                    RestartPolicy::Permanent,
                    ChildBehavior::Healthy,
                    healthy_counter.clone(),
                ),
                make_child_spec(
                    "multi_temp_fail",
                    RestartPolicy::Temporary,
                    ChildBehavior::DelayedFail { ms: 100 },
                    failing_counter.clone(),
                ),
            ],
            restart_budget: test_budget(5),
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_multi_indep".to_string()), Supervisor, config)
                .await
                .unwrap();

        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);

        // Healthy child spawned once and still running
        assert_eq!(healthy_counter.load(Ordering::SeqCst), 1);
        // Temporary child failed once, not restarted
        assert_eq!(failing_counter.load(Ordering::SeqCst), 1);

        // Supervisor still has the healthy child
        let running: Vec<_> = sup_ref
            .get_children()
            .into_iter()
            .filter(|c| c.get_status() == ActorStatus::Running)
            .collect();
        assert_eq!(running.len(), 1);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }

    #[tokio::test]
    async fn window_expiry_allows_more_restarts() {
        let counter = Arc::new(AtomicU32::new(0));

        // Budget: 1 restart in a 200ms window (no reset_after)
        let budget = RestartBudget {
            max_restarts: 1,
            max_window: Duration::from_millis(200),
            reset_after: None,
        };

        let config = SupervisorConfig {
            children: vec![make_child_spec(
                "window_child",
                RestartPolicy::Permanent,
                ChildBehavior::DelayedFail { ms: 300 },
                counter.clone(),
            )],
            restart_budget: budget,
            retry_strategy: fast_retry(),
        };

        let (sup_ref, sup_handle) =
            Actor::spawn(Some("test_window_expiry".to_string()), Supervisor, config)
                .await
                .unwrap();

        // Each child lives ~300ms, then fails. Window is 200ms, so each failure starts a new window.
        // With budget=1 per window, each failure is restart #1 in a fresh window => no meltdown.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert_eq!(sup_ref.get_status(), ActorStatus::Running);
        assert!(counter.load(Ordering::SeqCst) >= 3);

        sup_ref.stop(None);
        let _ = sup_handle.await;
    }
}
