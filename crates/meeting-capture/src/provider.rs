use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use futures_core::Stream;
use serde::{Deserialize, Serialize};

use crate::{CaptureEvent, CaptureProviderKind, MeetingBot, StartCaptureRequest};

pub type ProviderResult<T> = Result<T, ProviderError>;
pub type ProviderFuture<'a, T> = Pin<Box<dyn Future<Output = ProviderResult<T>> + Send + 'a>>;

pub struct CaptureEventStream(
    Pin<Box<dyn Stream<Item = ProviderResult<CaptureEvent>> + Send + 'static>>,
);

impl CaptureEventStream {
    pub fn new(stream: impl Stream<Item = ProviderResult<CaptureEvent>> + Send + 'static) -> Self {
        Self(Box::pin(stream))
    }
}

impl Stream for CaptureEventStream {
    type Item = ProviderResult<CaptureEvent>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.as_mut().poll_next(cx)
    }
}

pub trait MeetingCaptureProvider: Send + Sync {
    fn kind(&self) -> CaptureProviderKind;

    fn start(&self, request: StartCaptureRequest) -> ProviderFuture<'_, MeetingBot>;

    fn stop<'a>(&'a self, bot_id: &'a str) -> ProviderFuture<'a, ()>;

    fn status<'a>(&'a self, bot_id: &'a str) -> ProviderFuture<'a, MeetingBot>;

    fn events<'a>(
        &'a self,
        bot_id: &'a str,
        after_sequence: Option<u64>,
    ) -> ProviderFuture<'a, CaptureEventStream>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{kind:?}: {message}")]
pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub message: String,
    pub retryable: bool,
}

impl ProviderError {
    pub fn new(kind: ProviderErrorKind, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderErrorKind {
    InvalidRequest,
    Authentication,
    Unsupported,
    RateLimited,
    Unavailable,
    Capacity,
    NotFound,
    Conflict,
    Internal,
}
