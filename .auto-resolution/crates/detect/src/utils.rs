use std::future::Future;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::{sync::oneshot, task::JoinHandle};

pub struct BackgroundTask {
    running: Arc<AtomicBool>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    task_handle: Option<JoinHandle<()>>,
}

impl Default for BackgroundTask {
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            shutdown_tx: None,
            task_handle: None,
        }
    }
}

impl BackgroundTask {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn set_running(&self, value: bool) {
        self.running.store(value, Ordering::SeqCst)
    }

    pub fn get_running_state(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }

    pub fn start<F, Fut>(&mut self, task_fn: F)
    where
        F: FnOnce(Arc<AtomicBool>, oneshot::Receiver<()>) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if self.is_running() {
            return;
        }
        self.set_running(true);
        let running = self.get_running_state();

        let (tx, rx) = oneshot::channel();
        self.shutdown_tx = Some(tx);

        self.task_handle = Some(tokio::spawn(async move {
            task_fn(running, rx).await;
        }));
    }

    pub fn stop(&mut self) {
        if !self.is_running() {
            return;
        }

        self.set_running(false);

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        self.task_handle.take();
    }
}
