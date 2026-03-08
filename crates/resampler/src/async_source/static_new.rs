use futures_util::{Stream, pin_mut};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

use hypr_audio_interface::AsyncSource;

use crate::{Async, FixedAsync, PolynomialDegree, RubatoChunkResampler};

pub trait AsyncSourceChunkResampleExt: AsyncSource + Sized {
    fn resampled_chunks(
        self,
        target_rate: u32,
        output_chunk_size: usize,
    ) -> Result<ResamplerStaticNew<Self>, crate::Error> {
        ResamplerStaticNew::new(self, target_rate, output_chunk_size)
    }
}

impl<T> AsyncSourceChunkResampleExt for T where T: AsyncSource + Sized {}

#[pin_project]
pub struct ResamplerStaticNew<S>
where
    S: AsyncSource,
{
    source: S,
    driver: RubatoChunkResampler<Async<f32>, 1>,
    finished: bool,
}

impl<S> ResamplerStaticNew<S>
where
    S: AsyncSource,
{
    pub fn new(
        source: S,
        target_rate: u32,
        output_chunk_size: usize,
    ) -> Result<Self, crate::Error> {
        let driver = Self::build_driver(&source, target_rate, output_chunk_size)?;

        Ok(Self {
            source,
            driver,
            finished: false,
        })
    }

    fn build_driver(
        source: &S,
        target_rate: u32,
        output_chunk_size: usize,
    ) -> Result<RubatoChunkResampler<Async<f32>, 1>, crate::Error> {
        let source_rate = source.sample_rate();
        let input_block_size = output_chunk_size;
        let ratio = target_rate as f64 / source_rate as f64;

        let resampler = Async::<f32>::new_poly(
            ratio,
            2.0,
            PolynomialDegree::Cubic,
            input_block_size.max(1),
            1,
            FixedAsync::Input,
        )?;

        let driver = RubatoChunkResampler::new(resampler, output_chunk_size, input_block_size);
        Ok(driver)
    }

    fn finalize(&mut self) -> Result<(), crate::Error> {
        if self.finished {
            return Ok(());
        }

        self.driver.process_all_ready_blocks()?;

        if self.driver.has_input() {
            self.driver.process_partial_block(true)?;
        }

        self.finished = true;
        Ok(())
    }
}

impl<S> Stream for ResamplerStaticNew<S>
where
    S: AsyncSource,
{
    type Item = Result<Vec<f32>, crate::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();

        loop {
            if let Some(chunk) = me.driver.take_full_chunk() {
                return Poll::Ready(Some(Ok(chunk)));
            }

            if me.finished {
                return Poll::Ready(me.driver.take_all_output().map(Ok));
            }

            match me.driver.process_one_block() {
                Ok(true) => continue,
                Ok(false) => {}
                Err(err) => return Poll::Ready(Some(Err(err))),
            }

            let sample_poll = {
                let inner = me.source.as_stream();
                pin_mut!(inner);
                inner.poll_next(cx)
            };

            match sample_poll {
                Poll::Ready(Some(sample)) => {
                    me.driver.push_sample(sample);
                }
                Poll::Ready(None) => {
                    if let Err(err) = me.finalize() {
                        return Poll::Ready(Some(Err(err)));
                    }
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
