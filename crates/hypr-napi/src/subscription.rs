use napi_derive::napi;

/// Handle returned from `subscribe`.
///
/// Holds the cancellation sender for the spawned watch loop plus the one-time
/// reactivity analysis result. Call `unsubscribe()` from JS (or let GC take the
/// handle) to stop receiving deltas; the watch task drops its live-query
/// registration on the way out. `reactive` is false when the query plan could
/// not be statically analyzed, matching Tauri's `SubscriptionRegistration`.
#[napi]
pub struct SubscriptionHandle {
    cancel: Option<tokio::sync::oneshot::Sender<()>>,
    reactive: bool,
}

impl SubscriptionHandle {
    pub(crate) fn new(cancel: tokio::sync::oneshot::Sender<()>, reactive: bool) -> Self {
        Self {
            cancel: Some(cancel),
            reactive,
        }
    }
}

#[napi]
impl SubscriptionHandle {
    /// One-time analysis: `true` when the query plan is reactive, `false`
    /// for non-reactive SQL (e.g. CTEs we cannot trace). Static for the
    /// lifetime of the handle.
    #[napi(getter)]
    pub fn reactive(&self) -> bool {
        self.reactive
    }

    /// Stop the underlying watch. Safe to call more than once.
    #[napi]
    pub fn unsubscribe(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            let _ = cancel.send(());
        }
    }
}
