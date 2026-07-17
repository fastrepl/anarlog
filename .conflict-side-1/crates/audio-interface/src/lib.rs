use futures_util::Stream;

pub trait AsyncSource {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_;

    fn sample_rate(&self) -> u32;
}

#[cfg(feature = "rodio")]
impl<S: rodio::Source> AsyncSource for S {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        let channels = self.channels().get() as usize;
        futures_util::stream::iter(self.by_ref().step_by(channels))
    }

    fn sample_rate(&self) -> u32 {
        rodio::Source::sample_rate(self).into()
    }
}
