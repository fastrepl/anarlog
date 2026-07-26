use libpulse_binding as pulse;
use libpulse_binding::context::{Context, FlagSet as ContextFlagSet};
use libpulse_binding::mainloop::threaded::Mainloop;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::{BackgroundTask, DetectEvent};

#[derive(Default)]
pub struct Detector {
    background: BackgroundTask,
}

struct DetectorState {
    last_state: bool,
    last_change: Instant,
    debounce_duration: Duration,
}

impl DetectorState {
    fn new() -> Self {
        Self {
            last_state: false,
            last_change: Instant::now(),
            debounce_duration: Duration::from_millis(500),
        }
    }

    fn should_trigger(&mut self, new_state: bool) -> bool {
        let now = Instant::now();

        if new_state == self.last_state {
            return false;
        }
        if now.duration_since(self.last_change) < self.debounce_duration {
            return false;
        }

        self.last_state = new_state;
        self.last_change = now;
        true
    }
}

impl crate::Observer for Detector {
    fn start(&mut self, f: crate::DetectCallback) {
        self.background.start(|running, mut rx| async move {
            let (tx, mut notify_rx) = tokio::sync::mpsc::channel(1);

            std::thread::spawn(move || {
                let callback = Arc::new(Mutex::new(f));
                let detector_state = Arc::new(Mutex::new(DetectorState::new()));

                let mut mainloop = match Mainloop::new() {
                    Some(m) => m,
                    None => {
                        tracing::error!("failed_to_create_pulseaudio_mainloop");
                        return;
                    }
                };

                let mut context = match Context::new(&mainloop, "hyprnote-mic-detector") {
                    Some(c) => c,
                    None => {
                        tracing::error!("failed_to_create_pulseaudio_context");
                        return;
                    }
                };

                let callback_for_subscribe = callback.clone();
                let detector_state_for_subscribe = detector_state.clone();

                context.set_subscribe_callback(Some(Box::new(
                    move |facility, operation, _index| {
                        if let Some(pulse::context::subscribe::Facility::Source) = facility
                            && let Some(pulse::context::subscribe::Operation::Changed) = operation
                            && let Ok(mut state) = detector_state_for_subscribe.lock()
                        {
                            let mic_in_use = check_mic_in_use();

                            if state.should_trigger(mic_in_use) {
                                if mic_in_use {
                                    let cb = callback_for_subscribe.clone();
                                    std::thread::spawn(move || {
                                        let apps = crate::list_mic_using_apps().unwrap_or_default();
                                        tracing::info!("mic_started_detected: {:?}", apps);

                                        if let Ok(guard) = cb.lock() {
                                            let event = DetectEvent::MicStarted(apps);
                                            tracing::info!(event = ?event, "detected");
                                            (*guard)(event);
                                        }
                                    });
                                } else if let Ok(guard) = callback_for_subscribe.lock() {
                                    let event = DetectEvent::MicStopped(vec![]);
                                    tracing::info!(event = ?event, "detected");
                                    (*guard)(event);
                                }
                            }
                        }
                    },
                )));

                if context
                    .connect(None, ContextFlagSet::NOFLAGS, None)
                    .is_err()
                {
                    tracing::error!("failed_to_connect_to_pulseaudio");
                    return;
                }

                if mainloop.start().is_err() {
                    tracing::error!("failed_to_start_mainloop");
                    return;
                }

                mainloop.lock();
                loop {
                    match context.get_state() {
                        pulse::context::State::Ready => break,
                        pulse::context::State::Failed | pulse::context::State::Terminated => {
                            mainloop.unlock();
                            tracing::error!("pulseaudio_context_connection_failed");
                            return;
                        }
                        _ => {
                            mainloop.wait();
                        }
                    }
                }
                mainloop.unlock();

                tracing::info!("pulseaudio_context_connected");

                context.subscribe(
                    pulse::context::subscribe::InterestMaskSet::SOURCE,
                    |success| {
                        if success {
                            tracing::info!("subscribed_to_pulseaudio_source_events");
                        } else {
                            tracing::error!("failed_to_subscribe_to_pulseaudio_events");
                        }
                    },
                );

                let initial_mic_state = check_mic_in_use();
                if let Ok(mut state) = detector_state.lock() {
                    state.last_state = initial_mic_state;
                }

                let _ = tx.blocking_send(());

                loop {
                    std::thread::park();
                }
            });

            let _ = notify_rx.recv().await;

            loop {
                tokio::select! {
                    _ = &mut rx => {
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                        if !running.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            }
        });
    }

    fn stop(&mut self) {
        self.background.stop();
    }
}

fn check_mic_in_use() -> bool {
    let mut mainloop = match Mainloop::new() {
        Some(m) => m,
        None => return false,
    };

    let mut context = match Context::new(&mainloop, "hyprnote-mic-check") {
        Some(c) => c,
        None => return false,
    };

    let result = Arc::new(Mutex::new(false));
    let result_clone = result.clone();

    if context
        .connect(None, ContextFlagSet::NOFLAGS, None)
        .is_err()
    {
        return false;
    }

    if mainloop.start().is_err() {
        return false;
    }

    mainloop.lock();
    loop {
        match context.get_state() {
            pulse::context::State::Ready => break,
            pulse::context::State::Failed | pulse::context::State::Terminated => {
                mainloop.unlock();
                return false;
            }
            _ => {
                mainloop.wait();
            }
        }
    }
    mainloop.unlock();

    let introspector = context.introspect();
    let done = Arc::new(Mutex::new(false));
    let done_clone = done.clone();

    introspector.get_source_output_info_list(move |list_result| match list_result {
        pulse::callbacks::ListResult::Item(info) => {
            if !info.corked
                && let Ok(mut r) = result_clone.lock()
            {
                *r = true;
            }
        }
        pulse::callbacks::ListResult::End => {
            if let Ok(mut d) = done_clone.lock() {
                *d = true;
            }
        }
        pulse::callbacks::ListResult::Error => {
            if let Ok(mut d) = done_clone.lock() {
                *d = true;
            }
        }
    });

    for _ in 0..100 {
        if let Ok(d) = done.lock()
            && *d
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    mainloop.stop();

    result.lock().map(|r| *r).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Observer, new_callback};

    #[tokio::test]
    async fn test_detector() {
        let mut detector = Detector::default();
        detector.start(new_callback(|v| {
            println!("{:?}", v);
        }));

        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        detector.stop();
    }
}
