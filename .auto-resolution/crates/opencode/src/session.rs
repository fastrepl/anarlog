use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use futures_util::Stream;
use futures_util::StreamExt;
use tokio_util::sync::CancellationToken;

use crate::error::Error;
use crate::events::{Event, Input, RunStreamedResult, SessionTurn};
use crate::exec::{OpencodeExec, OpencodeExecArgs};
use crate::options::{OpencodeOptions, SessionOptions, TurnOptions};

#[derive(Debug, Clone)]
pub struct Opencode {
    exec: Arc<OpencodeExec>,
    options: OpencodeOptions,
}

impl Opencode {
    pub fn new(options: OpencodeOptions) -> Self {
        let exec = OpencodeExec::new(options.opencode_path_override.clone(), options.env.clone());
        Self {
            exec: Arc::new(exec),
            options,
        }
    }

    pub fn start_session(&self, options: SessionOptions) -> Session {
        Session::new(self.exec.clone(), self.options.clone(), options, None)
    }

    pub fn continue_last_session(&self, options: SessionOptions) -> Session {
        let mut options = options;
        options.continue_last = true;
        Session::new(self.exec.clone(), self.options.clone(), options, None)
    }

    pub fn resume_session(&self, id: impl Into<String>, options: SessionOptions) -> Session {
        Session::new(
            self.exec.clone(),
            self.options.clone(),
            options,
            Some(id.into()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct Session {
    exec: Arc<OpencodeExec>,
    _options: OpencodeOptions,
    session_options: SessionOptions,
    id: Arc<Mutex<Option<String>>>,
}

impl Session {
    fn new(
        exec: Arc<OpencodeExec>,
        options: OpencodeOptions,
        session_options: SessionOptions,
        id: Option<String>,
    ) -> Self {
        Self {
            exec,
            _options: options,
            session_options,
            id: Arc::new(Mutex::new(id)),
        }
    }

    pub fn id(&self) -> Result<Option<String>, Error> {
        self.id
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| Error::Poisoned)
    }

    pub async fn run_streamed<I>(
        &self,
        input: I,
        turn_options: TurnOptions,
    ) -> Result<RunStreamedResult, Error>
    where
        I: Into<Input>,
    {
        let input = input.into();
        let (prompt, input_files) = input.normalize();
        let thread_id = self.id()?;
        let mut files = self.session_options.files.clone();
        files.extend(input_files);

        let stream = self.exec.run(OpencodeExecArgs {
            input: prompt,
            session_id: thread_id,
            continue_last: self.session_options.continue_last,
            fork: self.session_options.fork,
            model: self.session_options.model.clone(),
            agent: self.session_options.agent.clone(),
            hostname: self.session_options.hostname.clone(),
            port: self.session_options.port,
            working_directory: self.session_options.working_directory.clone(),
            files,
            cancellation_token: turn_options.cancellation_token,
        })?;

        Ok(RunStreamedResult {
            events: Box::pin(ManagedEventStream {
                inner: stream.events,
                session_id: self.id.clone(),
                shutdown: stream.shutdown,
            }),
        })
    }

    pub async fn run<I>(&self, input: I, turn_options: TurnOptions) -> Result<SessionTurn, Error>
    where
        I: Into<Input>,
    {
        let streamed = self.run_streamed(input, turn_options).await?;
        let mut events = streamed.events;
        let mut items = Vec::new();

        while let Some(event) = events.next().await {
            items.push(event?);
        }

        Ok(SessionTurn { events: items })
    }
}

struct ManagedEventStream {
    inner: crate::events::EventStream,
    session_id: Arc<Mutex<Option<String>>>,
    shutdown: CancellationToken,
}

impl Stream for ManagedEventStream {
    type Item = Result<Event, Error>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        match self.inner.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(event))) => {
                if let Some(session_id) = event.session_id() {
                    if let Ok(mut guard) = self.session_id.lock() {
                        *guard = Some(session_id.to_string());
                    }
                }
                Poll::Ready(Some(Ok(event)))
            }
            other => other,
        }
    }
}

impl Drop for ManagedEventStream {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}
