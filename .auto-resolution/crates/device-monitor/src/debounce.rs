use std::collections::VecDeque;
use std::sync::mpsc;
use std::time::{Duration, Instant};

struct PendingEvent<T, K> {
    event: T,
    key: K,
    release_at: Instant,
}

pub struct EventBuffer<T, K, F>
where
    F: Fn(&T) -> K,
{
    delay: Duration,
    events: VecDeque<PendingEvent<T, K>>,
    key_fn: F,
}

pub enum State<T> {
    Ready(T),
    Wait(Duration),
    Empty,
}

impl<T, K, F> EventBuffer<T, K, F>
where
    K: PartialEq,
    F: Fn(&T) -> K,
{
    pub fn new(delay: Duration, key_fn: F) -> Self {
        Self {
            delay,
            events: VecDeque::new(),
            key_fn,
        }
    }

    pub fn put(&mut self, event: T) {
        let now = Instant::now();
        let key = (self.key_fn)(&event);

        self.events.retain(|e| e.release_at > now && e.key != key);

        self.events.push_back(PendingEvent {
            event,
            key,
            release_at: now + self.delay,
        });
    }

    pub fn get(&mut self) -> State<T> {
        let now = Instant::now();
        match self.events.front() {
            None => State::Empty,
            Some(e) if e.release_at > now => State::Wait(e.release_at - now),
            Some(_) => State::Ready(self.events.pop_front().unwrap().event),
        }
    }
}

pub fn spawn_debounced_by_key<T, K, F>(
    delay: Duration,
    raw_rx: mpsc::Receiver<T>,
    debounced_tx: mpsc::Sender<T>,
    key_fn: F,
) where
    T: Send + 'static,
    K: PartialEq + Send + 'static,
    F: Fn(&T) -> K + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buffer = EventBuffer::new(delay, key_fn);

        loop {
            let timeout = match buffer.get() {
                State::Ready(event) => {
                    let _ = debounced_tx.send(event);
                    continue;
                }
                State::Wait(duration) => duration,
                State::Empty => Duration::from_secs(60),
            };

            match raw_rx.recv_timeout(timeout) {
                Ok(event) => {
                    buffer.put(event);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    while let State::Ready(event) = buffer.get() {
                        let _ = debounced_tx.send(event);
                    }
                    break;
                }
            }
        }
    });
}

use crate::{DeviceEvent, DeviceSwitch};

pub fn spawn_device_event_debouncer(
    delay: Duration,
    raw_rx: mpsc::Receiver<DeviceEvent>,
    debounced_tx: mpsc::Sender<DeviceEvent>,
) {
    std::thread::spawn(move || {
        let mut buffer: EventBuffer<DeviceSwitch, u8, _> =
            EventBuffer::new(delay, |switch: &DeviceSwitch| match switch {
                DeviceSwitch::DefaultInputChanged => 0u8,
                DeviceSwitch::DefaultOutputChanged { .. } => 1u8,
                DeviceSwitch::DeviceListChanged => 2u8,
            });

        loop {
            let timeout = match buffer.get() {
                State::Ready(switch) => {
                    let _ = debounced_tx.send(DeviceEvent::Switch(switch));
                    continue;
                }
                State::Wait(duration) => duration,
                State::Empty => Duration::from_secs(60),
            };

            match raw_rx.recv_timeout(timeout) {
                Ok(DeviceEvent::Switch(switch)) => {
                    buffer.put(switch);
                }
                Ok(update @ DeviceEvent::Update(_)) => {
                    let _ = debounced_tx.send(update);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    while let State::Ready(switch) = buffer.get() {
                        let _ = debounced_tx.send(DeviceEvent::Switch(switch));
                    }
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum TestEvent {
        Simple,
        WithPayload { value: i32 },
    }

    fn test_event_key(event: &TestEvent) -> u8 {
        match event {
            TestEvent::Simple => 0,
            TestEvent::WithPayload { .. } => 1,
        }
    }

    #[test]
    fn test_event_buffer_wait() {
        let mut buffer = EventBuffer::new(Duration::from_millis(20), test_event_key);
        buffer.put(TestEvent::Simple);
        assert!(matches!(buffer.get(), State::Wait(_)));
        thread::sleep(Duration::from_millis(10));
        assert!(matches!(buffer.get(), State::Wait(_)));
        thread::sleep(Duration::from_millis(15));
        assert!(matches!(buffer.get(), State::Ready(_)));
    }

    #[test]
    fn test_event_buffer_deduplication_by_key() {
        let mut buffer = EventBuffer::new(Duration::from_millis(20), test_event_key);
        buffer.put(TestEvent::Simple);
        buffer.put(TestEvent::WithPayload { value: 1 });
        thread::sleep(Duration::from_millis(10));
        buffer.put(TestEvent::Simple);
        thread::sleep(Duration::from_millis(25));

        let mut results = Vec::new();
        while let State::Ready(event) = buffer.get() {
            results.push(event);
        }

        assert_eq!(results.len(), 2);
        assert!(matches!(results[0], TestEvent::WithPayload { .. }));
        assert!(matches!(results[1], TestEvent::Simple));
    }

    #[test]
    fn test_event_buffer_preserves_latest_payload() {
        let mut buffer = EventBuffer::new(Duration::from_millis(20), test_event_key);
        buffer.put(TestEvent::WithPayload { value: 1 });
        buffer.put(TestEvent::WithPayload { value: 2 });
        buffer.put(TestEvent::WithPayload { value: 3 });
        thread::sleep(Duration::from_millis(25));

        let mut results = Vec::new();
        while let State::Ready(event) = buffer.get() {
            results.push(event);
        }

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], TestEvent::WithPayload { value: 3 });
    }

    #[test]
    fn test_event_buffer_different_keys_not_deduplicated() {
        let mut buffer = EventBuffer::new(Duration::from_millis(20), test_event_key);
        buffer.put(TestEvent::Simple);
        buffer.put(TestEvent::WithPayload { value: 42 });
        thread::sleep(Duration::from_millis(25));

        let mut results = Vec::new();
        while let State::Ready(event) = buffer.get() {
            results.push(event);
        }

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_spawn_debounced_by_key() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (debounced_tx, debounced_rx) = mpsc::channel();

        spawn_debounced_by_key(
            Duration::from_millis(50),
            raw_rx,
            debounced_tx,
            test_event_key,
        );

        raw_tx.send(TestEvent::Simple).unwrap();
        raw_tx.send(TestEvent::Simple).unwrap();
        raw_tx.send(TestEvent::Simple).unwrap();

        thread::sleep(Duration::from_millis(100));

        let results: Vec<_> = debounced_rx.try_iter().collect();
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], TestEvent::Simple));
    }

    #[test]
    fn test_spawn_debounced_by_key_preserves_latest_payload() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (debounced_tx, debounced_rx) = mpsc::channel();

        spawn_debounced_by_key(
            Duration::from_millis(50),
            raw_rx,
            debounced_tx,
            test_event_key,
        );

        raw_tx.send(TestEvent::WithPayload { value: 1 }).unwrap();
        raw_tx.send(TestEvent::WithPayload { value: 2 }).unwrap();

        thread::sleep(Duration::from_millis(100));

        let results: Vec<_> = debounced_rx.try_iter().collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], TestEvent::WithPayload { value: 2 });
    }

    #[test]
    fn test_spawn_debounced_with_eq() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (debounced_tx, debounced_rx) = mpsc::channel();

        spawn_debounced_by_key(
            Duration::from_millis(50),
            raw_rx,
            debounced_tx,
            |e: &TestEvent| e.clone(),
        );

        raw_tx.send(TestEvent::Simple).unwrap();
        raw_tx.send(TestEvent::Simple).unwrap();
        raw_tx.send(TestEvent::Simple).unwrap();

        thread::sleep(Duration::from_millis(100));

        let results: Vec<_> = debounced_rx.try_iter().collect();
        assert_eq!(results.len(), 1);
        assert!(matches!(results[0], TestEvent::Simple));
    }

    #[test]
    fn test_spawn_debounced_with_eq_different_payloads_not_deduplicated() {
        let (raw_tx, raw_rx) = mpsc::channel();
        let (debounced_tx, debounced_rx) = mpsc::channel();

        spawn_debounced_by_key(
            Duration::from_millis(50),
            raw_rx,
            debounced_tx,
            |e: &TestEvent| e.clone(),
        );

        raw_tx.send(TestEvent::WithPayload { value: 1 }).unwrap();
        raw_tx.send(TestEvent::WithPayload { value: 2 }).unwrap();

        thread::sleep(Duration::from_millis(100));

        let results: Vec<_> = debounced_rx.try_iter().collect();
        assert_eq!(results.len(), 2);
    }
}
