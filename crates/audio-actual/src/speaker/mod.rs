use anyhow::Result;
#[cfg(any(test, not(target_os = "macos")))]
use futures_util::Stream;
#[cfg(any(test, not(target_os = "macos")))]
use pin_project::pin_project;

pub(super) const CHUNK_SIZE: usize = 256;
pub(super) const BUFFER_SIZE: usize = CHUNK_SIZE * 256;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
type PlatformSpeakerInput = macos::SpeakerInput;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
type PlatformSpeakerInput = windows::SpeakerInput;
#[cfg(all(target_os = "windows", not(test)))]
type PlatformSpeakerStream = windows::SpeakerStream;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
type PlatformSpeakerInput = linux::SpeakerInput;
#[cfg(all(target_os = "linux", not(test)))]
type PlatformSpeakerStream = linux::SpeakerStream;

#[cfg(test)]
mod mock;

#[cfg(all(any(target_os = "windows", target_os = "linux"), not(test)))]
type InnerStream = PlatformSpeakerStream;

#[cfg(test)]
type InnerStream = mock::MockInnerStream;

// https://github.com/floneum/floneum/blob/50afe10/interfaces/kalosm-sound/src/source/mic.rs#L41
pub struct SpeakerInput {
    inner: PlatformSpeakerInput,
}

#[cfg(all(target_os = "macos", not(test)))]
pub type SpeakerStream = macos::SpeakerStream;

impl SpeakerInput {
    pub fn new() -> Result<Self> {
        let inner = PlatformSpeakerInput::new()?;
        Ok(Self { inner })
    }

    pub fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    #[cfg(all(target_os = "macos", not(test)))]
    pub fn stream(self) -> Result<SpeakerStream> {
        Ok(self.inner.stream())
    }

    #[cfg(all(not(target_os = "macos"), not(test)))]
    pub fn stream(self) -> Result<SpeakerStream> {
        let inner = self.inner.stream()?;
        let initial_rate = inner.sample_rate();
        Ok(SpeakerStream {
            inner,
            buffer: Vec::new(),
            buffer_idx: 0,
            buffer_rate: initial_rate,
        })
    }

    #[cfg(test)]
    pub fn stream(self) -> Result<SpeakerStream> {
        let initial_rate = self.inner.sample_rate();
        Ok(SpeakerStream {
            inner: mock::MockInnerStream::new(vec![], initial_rate),
            buffer: Vec::new(),
            buffer_idx: 0,
            buffer_rate: initial_rate,
        })
    }
}

// https://github.com/floneum/floneum/blob/50afe10/interfaces/kalosm-sound/src/source/mic.rs#L140
#[cfg(any(test, not(target_os = "macos")))]
#[pin_project]
pub struct SpeakerStream {
    #[pin]
    inner: InnerStream,
    buffer: Vec<f32>,
    buffer_idx: usize,
    buffer_rate: u32,
}

#[cfg(test)]
impl SpeakerStream {
    pub fn new_mock(
        chunks: Vec<Vec<f32>>,
        initial_rate: u32,
    ) -> (Self, std::sync::Arc<std::sync::atomic::AtomicU32>) {
        let mock = mock::MockInnerStream::new(chunks, initial_rate);
        let handle = mock.rate_handle();
        let rate = mock.sample_rate();
        (
            Self {
                inner: mock,
                buffer: Vec::new(),
                buffer_idx: 0,
                buffer_rate: rate,
            },
            handle,
        )
    }
}

#[cfg(any(test, not(target_os = "macos")))]
impl Stream for SpeakerStream {
    type Item = f32;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut this = self.project();

        if *this.buffer_idx < this.buffer.len() {
            let sample = this.buffer[*this.buffer_idx];
            *this.buffer_idx += 1;
            return std::task::Poll::Ready(Some(sample));
        }

        match this.inner.as_mut().poll_next(cx) {
            std::task::Poll::Ready(Some(chunk)) => {
                *this.buffer = chunk;
                *this.buffer_idx = 0;
                *this.buffer_rate = this.inner.sample_rate();
                if !this.buffer.is_empty() {
                    let sample = this.buffer[0];
                    *this.buffer_idx = 1;
                    std::task::Poll::Ready(Some(sample))
                } else {
                    std::task::Poll::Pending
                }
            }
            std::task::Poll::Ready(None) => std::task::Poll::Ready(None),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

#[cfg(any(test, not(target_os = "macos")))]
impl hypr_audio_interface::AsyncSource for SpeakerStream {
    fn as_stream(&mut self) -> impl Stream<Item = f32> + '_ {
        self
    }

    fn sample_rate(&self) -> u32 {
        self.buffer_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use hypr_audio_interface::AsyncSource;
    use serial_test::serial;
    use std::sync::atomic::Ordering;

    #[tokio::test]
    async fn test_buffer_rate_preserved_after_rate_change() {
        let (mut stream, rate_handle) = SpeakerStream::new_mock(
            vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]],
            48000,
        );

        assert_eq!(stream.sample_rate(), 48000);

        let s1 = stream.next().await.unwrap();
        assert_eq!(s1, 1.0);
        assert_eq!(stream.sample_rate(), 48000);

        rate_handle.store(44100, Ordering::Release);

        let s2 = stream.next().await.unwrap();
        assert_eq!(s2, 2.0);
        assert_eq!(
            stream.sample_rate(),
            48000,
            "Rate should stay 48000 for buffered samples"
        );

        let s3 = stream.next().await.unwrap();
        assert_eq!(s3, 3.0);
        assert_eq!(stream.sample_rate(), 48000);

        let s4 = stream.next().await.unwrap();
        assert_eq!(s4, 4.0);
        assert_eq!(stream.sample_rate(), 48000);

        let s5 = stream.next().await.unwrap();
        assert_eq!(s5, 5.0);
        assert_eq!(
            stream.sample_rate(),
            44100,
            "Rate should update to 44100 after new chunk is fetched"
        );

        let s6 = stream.next().await.unwrap();
        assert_eq!(s6, 6.0);
        assert_eq!(stream.sample_rate(), 44100);
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    #[serial]
    #[ignore = "requires audio hardware"]
    async fn test_macos() {
        use crate::play_sine_for_sec;

        let input = SpeakerInput::new().unwrap();
        let mut stream = input.stream().unwrap();

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let handle = play_sine_for_sec(2);

        let mut buffer = Vec::new();
        while let Some(sample) = stream.next().await {
            buffer.push(sample);
            if buffer.len() > 48000 {
                break;
            }
        }

        handle.join().unwrap();
        assert!(buffer.iter().any(|x| *x != 0.0));
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    #[serial]
    async fn test_windows() {
        let input = match SpeakerInput::new() {
            Ok(input) => input,
            Err(e) => {
                println!("Failed to create SpeakerInput: {}", e);
                return;
            }
        };

        let mut stream = match input.stream() {
            Ok(stream) => stream,
            Err(e) => {
                println!("Failed to create speaker stream: {}", e);
                return;
            }
        };

        let sample_rate = stream.sample_rate();
        assert!(sample_rate > 0);
        println!("Windows speaker sample rate: {}", sample_rate);

        let mut sample_count = 0;
        while let Some(_sample) = stream.next().await {
            sample_count += 1;
            if sample_count > 100 {
                break;
            }
        }

        assert!(sample_count > 0, "Should receive some audio samples");
        println!("Received {} samples from Windows speaker", sample_count);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[serial]
    #[ignore = "requires audio hardware"]
    async fn test_linux() {
        let input = match SpeakerInput::new() {
            Ok(input) => input,
            Err(e) => {
                println!("Failed to create SpeakerInput: {}", e);
                println!(
                    "This is expected if PulseAudio is not running or no audio devices are available"
                );
                return;
            }
        };

        let sample_rate = input.sample_rate();
        println!("Linux speaker sample rate: {}", sample_rate);
        assert!(sample_rate > 0);

        let mut stream = match input.stream() {
            Ok(stream) => stream,
            Err(e) => {
                println!("Failed to create speaker stream: {}", e);
                return;
            }
        };

        let stream_sample_rate = stream.sample_rate();
        println!("Linux speaker stream sample rate: {}", stream_sample_rate);
        assert!(stream_sample_rate > 0);

        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        let mut sample_count = 0;
        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(2));
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                _ = &mut timeout => {
                    println!("Timeout reached after collecting {} samples", sample_count);
                    break;
                }
                sample = stream.next() => {
                    if let Some(_s) = sample {
                        sample_count += 1;
                        if sample_count >= 1000 {
                            break;
                        }
                    } else {
                        break;
                    }
                }
            }
        }

        println!("Received {} samples from Linux speaker", sample_count);
        assert!(
            sample_count > 0,
            "Should receive audio samples from speaker monitor"
        );
    }
}
