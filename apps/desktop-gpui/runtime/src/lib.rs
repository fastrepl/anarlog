#![forbid(unsafe_code)]

pub mod deeplink;
mod library;
pub mod localization;
mod metrics;
mod queue;
mod shutdown;
mod startup;
mod types;
pub mod updater;
mod watch;

use std::{
    future::Future,
    panic::AssertUnwindSafe,
    path::PathBuf,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Instant,
};

use anlg_db_core::Db;
use anlg_db_execute::DbExecutor;
use futures::{FutureExt, future::BoxFuture};
use tokio::sync::{Semaphore, mpsc, oneshot, watch as state};
pub use tokio_util::sync::CancellationToken;

pub use metrics::RuntimeMetrics;
pub use shutdown::ShutdownPhase;
pub use types::*;
pub use watch::{DocumentWatch, MAX_WATCH_ROWS, QueryWatch, WatchSnapshot};

pub const QUEUE_CAPACITY: usize = 64;
pub const DATABASE_POOL_SIZE: u32 = 1;
pub const SERVICE_CAPACITY: usize = 16;
pub const MAX_WATCHES: usize = 32;
pub const MAX_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Profile {
    pub database: PathBuf,
}

#[derive(Clone, Debug)]
pub enum RuntimeState {
    Starting,
    Ready,
    Draining,
    Flushing,
    Delayed(ShutdownPhase),
    Closed(Result<()>),
}

#[derive(Clone)]
pub struct Services {
    pub db: Arc<Db>,
    pub executor: DbExecutor,
    pub shutdown_requested: CancellationToken,
    watch_slots: Arc<Semaphore>,
    watches: Arc<watch::WatchRegistry>,
    metrics: Arc<metrics::Metrics>,
    library: Arc<tokio::sync::Mutex<library::Cache>>,
}

type Work = Box<dyn FnOnce(Services) -> BoxFuture<'static, ()> + Send>;
pub type FlushParticipant = Box<dyn FnOnce() -> BoxFuture<'static, Result<()>> + Send>;

enum Command {
    Work(Work),
    Register(ShutdownPhase, FlushParticipant, oneshot::Sender<Result<()>>),
}

pub struct Reply<T>(oneshot::Receiver<Result<T>>);

impl<T> Reply<T> {
    pub async fn receive(self) -> Result<T> {
        self.0.await.map_err(|_| ServiceError::Closed)?
    }
}

#[derive(Clone)]
pub struct RuntimeHandle {
    sender: mpsc::Sender<Command>,
    service_sender: mpsc::Sender<Work>,
    admission: Arc<Mutex<bool>>,
    shutdown_requested: CancellationToken,
    state: state::Receiver<RuntimeState>,
    metrics: Arc<metrics::Metrics>,
}

impl RuntimeHandle {
    pub fn start(profile: Profile) -> std::io::Result<(Self, Reply<()>)> {
        let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
        let (service_sender, service_receiver) = mpsc::channel(SERVICE_CAPACITY);
        let (ready_tx, ready_rx) = oneshot::channel();
        let (state_tx, state) = state::channel(RuntimeState::Starting);
        let shutdown_requested = CancellationToken::new();
        let handle = Self {
            sender,
            service_sender,
            admission: Arc::new(Mutex::new(true)),
            shutdown_requested: shutdown_requested.clone(),
            state,
            metrics: Arc::new(metrics::Metrics::default()),
        };
        let worker_metrics = handle.metrics.clone();
        std::thread::Builder::new()
            .name("anarlog-runtime".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let error = types::failure(error);
                        let _ = ready_tx.send(Err(error.clone()));
                        state_tx.send_replace(RuntimeState::Closed(Err(error)));
                        return;
                    }
                };
                runtime.block_on(async move {
                    let startup = async {
                        let lease = startup::ProfileLease::acquire(&profile)?;
                        startup::check_reset(&profile)?;
                        let db = startup::open(&profile).await?;
                        Ok::<_, ServiceError>((
                            lease,
                            Services {
                                executor: DbExecutor::new(db.clone()),
                                watches: Arc::new(watch::WatchRegistry::new(db.clone())),
                                metrics: worker_metrics,
                                library: Arc::new(tokio::sync::Mutex::new(
                                    library::Cache::new(&profile.database).await?,
                                )),
                                db,
                                watch_slots: Arc::new(Semaphore::new(MAX_WATCHES)),
                                shutdown_requested,
                            },
                        ))
                    }
                    .await;
                    match startup {
                        Ok((lease, services)) => {
                            state_tx.send_replace(RuntimeState::Ready);
                            let _ = ready_tx.send(Ok(()));
                            let result =
                                queue::run(receiver, service_receiver, services, &state_tx).await;
                            drop(lease);
                            state_tx.send_replace(RuntimeState::Closed(result));
                        }
                        Err(error) => {
                            let _ = ready_tx.send(Err(error.clone()));
                            state_tx.send_replace(RuntimeState::Closed(Err(error)));
                        }
                    }
                });
            })?;
        Ok((handle, Reply(ready_rx)))
    }

    pub fn state(&self) -> state::Receiver<RuntimeState> {
        self.state.clone()
    }

    pub fn metrics(&self) -> RuntimeMetrics {
        self.metrics.snapshot()
    }

    /// Accepted work runs even when its reply is dropped. Use cancellation only for reads.
    pub fn submit<T, F, Fut>(&self, work: F) -> Result<Reply<T>>
    where
        T: Send + 'static,
        F: FnOnce(Services) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let (work, reply) = self.work(work);
        self.enqueue(Command::Work(work))?;
        Ok(reply)
    }

    /// Independent bounded jobs; durable producers must observe shutdown and finish persisting.
    pub fn service<T, F, Fut>(&self, work: F) -> Result<Reply<T>>
    where
        T: Send + 'static,
        F: FnOnce(Services) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let admission = self.admission.lock().map_err(types::failure)?;
        if !*admission {
            return Err(ServiceError::Closed);
        }
        self.ensure_ready()?;
        let permit = self
            .service_sender
            .try_reserve()
            .map_err(|error| self.queue_error(error))?;
        let (work, reply) = self.work(work);
        self.metrics.admit();
        permit.send(work);
        Ok(reply)
    }

    pub fn read<T, F, Fut>(&self, cancellation: CancellationToken, work: F) -> Result<Reply<T>>
    where
        T: Send + 'static,
        F: FnOnce(Services) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.submit(move |services| async move {
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(ServiceError::Cancelled),
                result = work(services) => result,
            }
        })
    }

    pub fn register_flush(&self, participant: FlushParticipant) -> Result<Reply<()>> {
        self.register_shutdown(ShutdownPhase::ApplicationState, participant)
    }

    /// FIFO barrier for already-admitted database work. Does not cancel late writes.
    pub fn flush(&self) -> Result<Reply<()>> {
        self.submit(|_| async { Ok(()) })
    }

    /// Draining belongs to the runtime, so dropping a waiter cannot interrupt it.
    pub async fn shutdown(&self) -> Result<()> {
        {
            let mut admission = self.admission.lock().map_err(types::failure)?;
            *admission = false;
            self.shutdown_requested.cancel();
        }
        let mut state = self.state.clone();
        loop {
            if let RuntimeState::Closed(result) = state.borrow_and_update().clone() {
                return result;
            }
            state.changed().await.map_err(|_| ServiceError::Closed)?;
        }
    }

    fn work<T, F, Fut>(&self, work: F) -> (Work, Reply<T>)
    where
        T: Send + 'static,
        F: FnOnce(Services) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();
        let enqueued = Instant::now();
        let metrics = self.metrics.clone();
        (
            Box::new(move |services| {
                Box::pin(async move {
                    metrics.start(enqueued.elapsed());
                    let started = Instant::now();
                    let result = AssertUnwindSafe(async move { work(services).await })
                        .catch_unwind()
                        .await
                        .unwrap_or_else(|_| Err(types::failure("Runtime operation panicked")));
                    metrics.complete(started.elapsed(), result.is_err());
                    if matches!(result, Err(ServiceError::Cancelled)) {
                        metrics.cancelled.fetch_add(1, Ordering::Relaxed);
                    }
                    let _ = tx.send(result);
                })
            }),
            Reply(rx),
        )
    }

    fn enqueue(&self, command: Command) -> Result<()> {
        let admission = self.admission.lock().map_err(types::failure)?;
        if !*admission {
            return Err(ServiceError::Closed);
        }
        self.ensure_ready()?;
        let permit = self
            .sender
            .try_reserve()
            .map_err(|error| self.queue_error(error))?;
        if matches!(command, Command::Work(_)) {
            self.metrics.admit();
        }
        permit.send(command);
        Ok(())
    }

    fn queue_error(&self, error: mpsc::error::TrySendError<()>) -> ServiceError {
        match error {
            mpsc::error::TrySendError::Full(_) => {
                self.metrics.busy.fetch_add(1, Ordering::Relaxed);
                ServiceError::Busy
            }
            mpsc::error::TrySendError::Closed(_) => ServiceError::Closed,
        }
    }

    fn ensure_ready(&self) -> Result<()> {
        match &*self.state.borrow() {
            RuntimeState::Ready => Ok(()),
            RuntimeState::Starting => {
                self.metrics.busy.fetch_add(1, Ordering::Relaxed);
                Err(ServiceError::Busy)
            }
            _ => Err(ServiceError::Closed),
        }
    }
}
