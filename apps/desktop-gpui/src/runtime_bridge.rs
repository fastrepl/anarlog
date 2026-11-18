use desktop_runtime::{Profile, Reply, RuntimeHandle, RuntimeState};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{App, Task};
use std::time::Duration;

pub fn start(profile: Profile) -> std::io::Result<(RuntimeHandle, Reply<()>)> {
    RuntimeHandle::start(profile)
}

/// Waits until the runtime accepts work; `false` once it failed to start or began shutting down.
pub async fn ready(runtime: &RuntimeHandle) -> bool {
    let mut state = runtime.state();
    loop {
        match *state.borrow_and_update() {
            RuntimeState::Ready => return true,
            RuntimeState::Starting => {}
            _ => return false,
        }
        if state.changed().await.is_err() {
            return false;
        }
    }
}

/// Call before App::quit, while windows and unsaved-draft recovery remain available.
pub fn drain_before_quit(
    runtime: RuntimeHandle,
    cx: &mut App,
) -> Task<desktop_runtime::Result<()>> {
    let deadline = cx.background_executor().timer(Duration::from_secs(12));
    cx.spawn(async move |_| {
        match select(runtime.shutdown().boxed(), deadline.boxed()).await {
            Either::Left((result, _)) => result,
            Either::Right(_) => Err(desktop_runtime::ServiceError::Failed(
                "Shutdown is still draining after 12 seconds; Anarlog quits once accepted writes finish.".into()
            )),
        }
    })
}
