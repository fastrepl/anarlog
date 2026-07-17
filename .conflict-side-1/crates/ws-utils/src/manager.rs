use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ConnectionManager {
    inner: Arc<Mutex<Option<CancellationToken>>>,
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(None)),
        }
    }
}

impl ConnectionManager {
    pub fn acquire_connection(&self) -> ConnectionGuard {
        let mut slot = self.inner.lock().unwrap();

        if let Some(old) = slot.take() {
            old.cancel();
        }

        let token = CancellationToken::new();
        *slot = Some(token.clone());

        ConnectionGuard { token }
    }
}

pub struct ConnectionGuard {
    token: CancellationToken,
}

impl ConnectionGuard {
    pub async fn cancelled(&self) {
        self.token.cancelled().await
    }
}
