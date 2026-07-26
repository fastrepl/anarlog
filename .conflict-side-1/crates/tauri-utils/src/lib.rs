use std::future::Future;

/// Run an async block synchronously by spawning a dedicated thread with its own
/// single-threaded Tokio runtime.
///
/// Tauri's plugin `.setup()` closure is synchronous, but it runs on a thread
/// that is already inside a Tokio runtime. Calling `block_on` directly (or via
/// `tauri::async_runtime::block_on`) panics with *"Cannot start a runtime from
/// within a runtime"*. Spawning a new OS thread sidesteps this because the new
/// thread is not part of any runtime.
///
/// Two patterns are common:
///
/// | Need | Function | Behaviour |
/// |------|----------|-----------|
/// | Result **must** be ready before setup returns (e.g. DB migrations) | [`block_on`] | Blocks the calling thread until the future completes |
/// | Work can finish in the background (e.g. legacy data import) | [`spawn`] | Fire-and-forget; errors are logged, never block setup |
pub fn block_on<F, T>(future: F) -> T
where
    F: Future<Output = T> + Send,
    T: Send,
{
    std::thread::scope(|s| {
        s.spawn(|| {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime");
            runtime.block_on(future)
        })
        .join()
        .expect("spawned thread panicked")
    })
}

/// Spawn an async block as a fire-and-forget background task.
///
/// Uses `tauri::async_runtime::spawn` under the hood, which schedules the
/// future onto the existing Tauri Tokio runtime — no extra threads or runtimes
/// are created. Errors are logged via `tracing::error!` and do not propagate.
///
/// Use this for non-critical initialization work that should not block app
/// startup (e.g. importing legacy data, warming caches).
pub fn spawn<F, E>(label: &'static str, future: F)
where
    F: Future<Output = Result<(), E>> + Send + 'static,
    E: std::fmt::Display + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(error) = future.await {
            tracing::error!("{label}: {error}");
        }
    });
}
