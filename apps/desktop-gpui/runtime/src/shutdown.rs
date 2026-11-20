use std::{
    any::Any,
    sync::{Arc, Weak},
    time::Duration,
};

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

pub(crate) struct Participant {
    pub(crate) phase: ShutdownPhase,
    owner: Option<Weak<dyn Any + Send + Sync>>,
    pub(crate) run: FlushParticipant,
}

impl Participant {
    pub(crate) fn live(&self) -> bool {
        self.owner
            .as_ref()
            .is_none_or(|owner| owner.strong_count() > 0)
    }
}

impl RuntimeHandle {
    pub fn register_shutdown(
        &self,
        phase: ShutdownPhase,
        participant: FlushParticipant,
    ) -> Result<Reply<()>> {
        self.register(Participant {
            phase,
            owner: None,
            run: participant,
        })
    }

    /// A flush participant that lasts only while `owner` does. Short-lived owners, such as
    /// one journal per editor, would otherwise fill the participant limit within a session.
    pub fn register_owned_flush<T: Any + Send + Sync>(
        &self,
        owner: &Arc<T>,
        participant: FlushParticipant,
    ) -> Result<Reply<()>> {
        let owner: Weak<dyn Any + Send + Sync> = Arc::downgrade(owner) as Weak<T>;
        self.register(Participant {
            phase: ShutdownPhase::ApplicationState,
            owner: Some(owner),
            run: participant,
        })
    }

    fn register(&self, participant: Participant) -> Result<Reply<()>> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.enqueue(crate::Command::Register(participant, sender))?;
        Ok(Reply(receiver))
    }
}
