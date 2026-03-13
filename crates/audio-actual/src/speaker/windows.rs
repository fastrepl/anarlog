use anyhow::Result;
use futures_util::Stream;
use pin_project::pin_project;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, mpsc};
use std::task::{Poll, Waker};
use std::thread;
use std::time::Duration;
use tracing::error;
use wasapi::{Direction, SampleType, StreamMode, WaveFormat, get_default_device};

use super::{BUFFER_SIZE, CHUNK_SIZE};

pub struct SpeakerInput {}

impl SpeakerInput {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn sample_rate(&self) -> u32 {
        44100
    }

    pub fn stream(self) -> SpeakerStream {
        let sample_queue = Arc::new(Mutex::new(VecDeque::new()));
        let waker_state = Arc::new(Mutex::new(WakerState {
            waker: None,
            has_data: false,
            shutdown: false,
        }));

        let queue_clone = sample_queue.clone();
        let waker_clone = waker_state.clone();
        let (init_tx, init_rx) = mpsc::channel();

        let capture_thread = thread::spawn(move || {
            if let Err(e) = SpeakerStream::capture_audio_loop(queue_clone, waker_clone, init_tx) {
                error!("Audio capture loop failed: {}", e);
            }
        });

        if let Ok(Err(e)) = init_rx.recv_timeout(Duration::from_secs(5)) {
            error!("Audio initialization failed: {}", e);
        }

        SpeakerStream {
            sample_queue,
            waker_state,
            capture_thread: Some(capture_thread),
        }
    }
}

struct WakerState {
    waker: Option<Waker>,
    has_data: bool,
    shutdown: bool,
}

#[pin_project(PinnedDrop)]
pub struct SpeakerStream {
    sample_queue: Arc<Mutex<VecDeque<f32>>>,
    waker_state: Arc<Mutex<WakerState>>,
    capture_thread: Option<thread::JoinHandle<()>>,
}

impl SpeakerStream {
    pub fn sample_rate(&self) -> u32 {
        44100
    }

    fn capture_audio_loop(
        sample_queue: Arc<Mutex<VecDeque<f32>>>,
        waker_state: Arc<Mutex<WakerState>>,
        init_tx: mpsc::Sender<Result<()>>,
    ) -> Result<()> {
        let init_result = (|| -> Result<_> {
            let device = get_default_device(&Direction::Render)?;
            let mut audio_client = device.get_iaudioclient()?;

            let desired_format = WaveFormat::new(32, 32, &SampleType::Float, 44100, 1, None);

            let (_def_time, min_time) = audio_client.get_device_period()?;

            let mode = StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: min_time,
            };

            audio_client.initialize_client(&desired_format, &Direction::Capture, &mode)?;

            let h_event = audio_client.set_get_eventhandle()?;
            let render_client = audio_client.get_audiocaptureclient()?;

            audio_client.start_stream()?;

            Ok((h_event, render_client))
        })();

        match init_result {
            Ok((h_event, render_client)) => {
                let _ = init_tx.send(Ok(()));

                loop {
                    {
                        let state = waker_state.lock().unwrap();
                        if state.shutdown {
                            break;
                        }
                    }

                    if h_event.wait_for_event(3000).is_err() {
                        error!("timeout error, stopping capture");
                        break;
                    }

                    let mut temp_queue = VecDeque::new();
                    if let Err(e) = render_client.read_from_device_to_deque(&mut temp_queue) {
                        error!("Failed to read audio data: {}", e);
                        continue;
                    }

                    if temp_queue.is_empty() {
                        continue;
                    }

                    let mut samples = Vec::new();
                    while temp_queue.len() >= 4 {
                        let bytes = [
                            temp_queue.pop_front().unwrap(),
                            temp_queue.pop_front().unwrap(),
                            temp_queue.pop_front().unwrap(),
                            temp_queue.pop_front().unwrap(),
                        ];
                        let sample = f32::from_le_bytes(bytes);
                        samples.push(sample);
                    }

                    if !samples.is_empty() {
                        {
                            let mut queue = sample_queue.lock().unwrap();
                            queue.extend(samples);

                            let len = queue.len();
                            if len > BUFFER_SIZE {
                                let dropped = len - BUFFER_SIZE;
                                tracing::warn!(dropped, "samples_dropped");
                                queue.drain(0..(len - BUFFER_SIZE));
                            }
                        }

                        {
                            let mut state = waker_state.lock().unwrap();
                            if !state.has_data {
                                state.has_data = true;
                                if let Some(waker) = state.waker.take() {
                                    drop(state);
                                    waker.wake();
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => {
                let _ = init_tx.send(Err(e));
                return Ok(());
            }
        }

        Ok(())
    }
}

#[pin_project::pinned_drop]
impl PinnedDrop for SpeakerStream {
    fn drop(self: std::pin::Pin<&mut Self>) {
        let this = self.project();

        {
            let mut state = this.waker_state.lock().unwrap();
            state.shutdown = true;
        }

        if let Some(thread) = this.capture_thread.take() {
            if let Err(e) = thread.join() {
                error!("Failed to join capture thread: {:?}", e);
            }
        }
    }
}

impl Stream for SpeakerStream {
    type Item = Vec<f32>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Option<Self::Item>> {
        let this = self.project();

        {
            let state = this.waker_state.lock().unwrap();
            if state.shutdown {
                return Poll::Ready(None);
            }
        }

        {
            let mut queue = this.sample_queue.lock().unwrap();
            if !queue.is_empty() {
                let chunk_len = queue.len().min(CHUNK_SIZE);
                let chunk: Vec<f32> = queue.drain(..chunk_len).collect();
                return Poll::Ready(Some(chunk));
            }
        }

        {
            let mut state = this.waker_state.lock().unwrap();
            if state.shutdown {
                return Poll::Ready(None);
            }
            state.has_data = false;
            state.waker = Some(cx.waker().clone());
            drop(state);
        }

        {
            let mut queue = this.sample_queue.lock().unwrap();
            if !queue.is_empty() {
                let chunk_len = queue.len().min(CHUNK_SIZE);
                let chunk: Vec<f32> = queue.drain(..chunk_len).collect();
                Poll::Ready(Some(chunk))
            } else {
                Poll::Pending
            }
        }
    }
}
