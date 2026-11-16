use desktop_runtime::{Profile, Reply, RuntimeHandle};
use futures::{
    FutureExt,
    future::{Either, select},
};
use gpui::{App, Task};
use std::time::Duration;

pub fn start(profile: Profile) -> std::io::Result<(RuntimeHandle, Reply<()>)> {
    RuntimeHandle::start(profile)
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
                "Shutdown is still draining after 12 seconds. Accepted writes remain active; wait and retry before quitting.".into()
            )),
        }
    })
}
