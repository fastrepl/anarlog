use std::panic::AssertUnwindSafe;

use futures::FutureExt;
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};

use crate::{
    Command, FlushParticipant, QUEUE_CAPACITY, Result, RuntimeState, ServiceError, Services,
    ShutdownPhase, Work, types::failure,
};

pub(crate) async fn run(
    receiver: mpsc::Receiver<Command>,
    service_receiver: mpsc::Receiver<Work>,
    services: Services,
    state: &watch::Sender<RuntimeState>,
) -> Result<()> {
    let (mut participants, jobs) = tokio::join!(
        database_queue(receiver, services.clone(), state),
        service_queue(service_receiver, services.clone()),
    );
    state.send_replace(RuntimeState::Flushing);
    let mut errors = Vec::new();
    if let Err(error) = jobs {
        errors.push(error.to_string());
    }
    participants.sort_by_key(|(phase, _)| *phase);
    for phase in [
        ShutdownPhase::PendingDeletions,
        ShutdownPhase::ApplicationState,
        ShutdownPhase::DatabaseFallback,
        ShutdownPhase::StopServices,
    ] {
        let mut pending = Vec::new();
        while participants.first().is_some_and(|(next, _)| *next == phase) {
            let (_, participant) = participants.remove(0);
            pending.push(AssertUnwindSafe(async move { participant().await }).catch_unwind());
        }
        let all = futures::future::join_all(pending);
        tokio::pin!(all);
        let results = match tokio::time::timeout(phase.budget(), &mut all).await {
            Ok(results) => results,
            Err(_) => {
                state.send_replace(RuntimeState::Delayed(phase));
                all.await
            }
        };
        for result in results {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => errors.push(error.to_string()),
                Err(_) => errors.push("Shutdown participant panicked".into()),
            }
        }
    }
    services.watches.close().await;
    services.db.pool().close().await;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(failure(errors.join("; ")))
    }
}

async fn database_queue(
    mut receiver: mpsc::Receiver<Command>,
    services: Services,
    state: &watch::Sender<RuntimeState>,
) -> Vec<(ShutdownPhase, FlushParticipant)> {
    let mut participants = Vec::new();
    let mut draining = false;
    loop {
        let command = tokio::select! {
            biased;
            _ = services.shutdown_requested.cancelled(), if !draining => {
                draining = true;
                receiver.close();
                state.send_replace(RuntimeState::Draining);
                continue;
            }
            command = receiver.recv() => command,
        };
        match command {
            Some(Command::Work(work)) => work(services.clone()).await,
            Some(Command::Register(phase, participant, reply)) => {
                if participants.len() == QUEUE_CAPACITY {
                    let _ = reply.send(Err(ServiceError::Busy));
                } else {
                    participants.push((phase, participant));
                    let _ = reply.send(Ok(()));
                }
            }
            None => break,
        }
    }
    services.shutdown_requested.cancel();
    participants
}

async fn service_queue(mut receiver: mpsc::Receiver<Work>, services: Services) -> Result<()> {
    let mut jobs = JoinSet::new();
    let mut draining = false;
    let mut ended = false;
    let mut errors = Vec::new();
    while !ended || !jobs.is_empty() {
        tokio::select! {
            biased;
            _ = services.shutdown_requested.cancelled(), if !draining => {
                draining = true;
                receiver.close();
            }
            result = jobs.join_next(), if !jobs.is_empty() => {
                if let Some(Err(error)) = result { errors.push(error.to_string()); }
            }
            work = receiver.recv(), if !ended && jobs.len() < 4 => {
                match work {
                    Some(work) => { jobs.spawn(work(services.clone())); }
                    None => ended = true,
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(failure(errors.join("; ")))
    }
}
