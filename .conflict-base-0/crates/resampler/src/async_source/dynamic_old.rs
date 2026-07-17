use std::pin::Pin;
use std::task::{Context, Poll};

use dasp::interpolate::Interpolator;
use futures_util::{Stream, pin_mut};
use hypr_audio_interface::AsyncSource;
use pin_project::pin_project;

#[pin_project]
pub struct ResamplerDynamicOld<S: AsyncSource> {
    source: S,
    target_sample_rate: u32,
    last_source_rate: u32,
    ratio: f64,

    phase: f64,

    interp: dasp::interpolate::linear::Linear<f32>,
    last_sample: f32,
    seeded: bool,
    bypass: bool,
}

impl<S: AsyncSource> ResamplerDynamicOld<S> {
    pub fn new(source: S, target_sample_rate: u32) -> Self {
        let initial_rate = source.sample_rate();
        let bypass = initial_rate == target_sample_rate;
        Self {
            source,
            target_sample_rate,
            last_source_rate: initial_rate,
            ratio: initial_rate as f64 / target_sample_rate as f64,
            phase: 0.0,
            interp: dasp::interpolate::linear::Linear::new(0.0, 0.0),
            last_sample: 0.0,
            seeded: false,
            bypass,
        }
    }

    #[inline]
    fn handle_rate_change(&mut self) {
        let new_rate = self.source.sample_rate();
        if new_rate == self.last_source_rate {
            return;
        }

        self.last_source_rate = new_rate;
        self.bypass = new_rate == self.target_sample_rate;
        self.ratio = new_rate as f64 / self.target_sample_rate as f64;
        self.phase = 0.0;
        self.interp = dasp::interpolate::linear::Linear::new(self.last_sample, self.last_sample);
    }
}

impl<S: AsyncSource> Stream for ResamplerDynamicOld<S> {
    type Item = f32;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();

        me.handle_rate_change();

        let inner = me.source.as_stream();
        pin_mut!(inner);

        if me.bypass {
            return inner.as_mut().poll_next(cx);
        }

        if !me.seeded {
            match inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(frame)) => {
                    me.last_sample = frame;
                    me.interp = dasp::interpolate::linear::Linear::new(frame, frame);
                    me.seeded = true;
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }

        while me.phase >= 1.0 {
            match inner.as_mut().poll_next(cx) {
                Poll::Ready(Some(frame)) => {
                    me.phase -= 1.0;
                    me.last_sample = frame;
                    me.interp.next_source_frame(frame);
                }
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Pending => return Poll::Pending,
            }
        }

        let out = me.interp.interpolate(me.phase);
        me.phase += me.ratio;
        Poll::Ready(Some(out))
    }
}

impl<S: AsyncSource> AsyncSource for ResamplerDynamicOld<S> {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        self
    }

    fn sample_rate(&self) -> u32 {
        self.target_sample_rate
    }
}
