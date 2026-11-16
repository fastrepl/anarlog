use std::time::Duration;

use crate::{FlushParticipant, Reply, Result, RuntimeHandle};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ShutdownPhase {
    PendingDeletions,
    ApplicationState,
    DatabaseFallback,
    StopServices,
}

impl ShutdownPhase {
    pub(crate) fn budget(self) -> Duration {
        match self {
            Self::PendingDeletions => Duration::from_secs(3),
            Self::ApplicationState => Duration::from_secs(5),
            Self::DatabaseFallback | Self::StopServices => Duration::from_secs(10),
        }
    }
}

impl RuntimeHandle {
    pub fn register_shutdown(
        &self,
        phase: ShutdownPhase,
        participant: FlushParticipant,
    ) -> Result<Reply<()>> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.enqueue(crate::Command::Register(phase, participant, sender))?;
        Ok(Reply(receiver))
    }
}
