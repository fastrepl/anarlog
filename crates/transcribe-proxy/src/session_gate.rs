use std::sync::Arc;

use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerDraining;

#[derive(Clone)]
pub struct SessionGate {
    inner: Arc<Inner>,
}

struct Inner {
    state: watch::Sender<State>,
}

#[derive(Clone, Copy)]
struct State {
    draining: bool,
    active: usize,
}

pub struct SessionPermit {
    inner: Arc<Inner>,
}

impl SessionGate {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                state: watch::channel(State {
                    draining: false,
                    active: 0,
                })
                .0,
            }),
        }
    }

    pub fn begin_drain(&self) {
        self.inner.state.send_if_modified(|state| {
            if state.draining {
                return false;
            }
            state.draining = true;
            true
        });
    }

    pub fn is_draining(&self) -> bool {
        self.inner.state.borrow().draining
    }

    pub fn active(&self) -> usize {
        self.inner.state.borrow().active
    }

    pub fn try_acquire(&self) -> Result<SessionPermit, ServerDraining> {
        let mut acquired = false;
        self.inner.state.send_if_modified(|state| {
            if state.draining {
                return false;
            }
            state.active += 1;
            acquired = true;
            true
        });
        if !acquired {
            return Err(ServerDraining);
        }

        Ok(SessionPermit {
            inner: self.inner.clone(),
        })
    }

    pub async fn wait_until_idle(&self) {
        let mut state = self.inner.state.subscribe();
        while state.borrow_and_update().active != 0 {
            if state.changed().await.is_err() {
                return;
            }
        }
    }
}

impl Default for SessionGate {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SessionPermit {
    fn drop(&mut self) {
        self.inner.release();
    }
}

impl Inner {
    fn release(&self) {
        self.state.send_modify(|state| {
            debug_assert!(state.active > 0);
            state.active -= 1;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn acquire_is_rejected_after_drain_starts() {
        let gate = SessionGate::new();
        let permit = gate.try_acquire().unwrap();
        assert_eq!(gate.active(), 1);

        gate.begin_drain();
        assert!(gate.is_draining());
        assert_eq!(gate.try_acquire().map(|_| ()), Err(ServerDraining));
        assert_eq!(gate.active(), 1);

        drop(permit);
        assert_eq!(gate.active(), 0);
    }

    #[tokio::test]
    async fn wait_until_idle_resolves_when_the_last_permit_drops() {
        let gate = SessionGate::new();
        let permit = gate.try_acquire().unwrap();
        gate.begin_drain();

        let wait = gate.wait_until_idle();
        tokio::pin!(wait);
        tokio::select! {
            _ = &mut wait => panic!("gate became idle while a session was still held"),
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }

        drop(permit);
        tokio::time::timeout(Duration::from_secs(1), wait)
            .await
            .expect("idle wait should finish after the last session ends");
    }

    #[tokio::test]
    async fn idle_transition_is_observed_before_the_waiter_subscribes() {
        let gate = SessionGate::new();
        let permit = gate.try_acquire().unwrap();
        gate.begin_drain();
        drop(permit);

        tokio::time::timeout(Duration::from_secs(1), gate.wait_until_idle())
            .await
            .expect("idle state should be retained for late waiters");
    }
}
