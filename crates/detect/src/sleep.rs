use std::sync::mpsc;
use std::thread::JoinHandle;

use block2::RcBlock;
use objc2::{msg_send, rc::Retained};
use objc2_app_kit::NSWorkspace;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSString};

use crate::{DetectCallback, DetectEvent};

struct SleepObserver {
    center: Retained<NSNotificationCenter>,
    will_sleep_observer: Retained<NSObject>,
    did_wake_observer: Retained<NSObject>,
}

impl Drop for SleepObserver {
    fn drop(&mut self) {
        unsafe {
            let _: () = msg_send![&*self.center, removeObserver: &*self.will_sleep_observer];
            let _: () = msg_send![&*self.center, removeObserver: &*self.did_wake_observer];
        }
    }
}

pub struct SleepDetector {
    shutdown_tx: Option<mpsc::Sender<()>>,
    thread_handle: Option<JoinHandle<()>>,
}

impl Default for SleepDetector {
    fn default() -> Self {
        Self {
            shutdown_tx: None,
            thread_handle: None,
        }
    }
}

impl crate::Observer for SleepDetector {
    fn start(&mut self, f: DetectCallback) {
        if self.thread_handle.is_some() {
            return;
        }

        let (shutdown_tx, shutdown_rx) = mpsc::channel();
        self.shutdown_tx = Some(shutdown_tx);

        self.thread_handle = Some(std::thread::spawn(move || {
            let will_sleep_callback = f.clone();
            let will_sleep_block = RcBlock::new(move |_notification: *const NSNotification| {
                will_sleep_callback(DetectEvent::SleepStateChanged { value: true });
            });

            let did_wake_callback = f.clone();
            let did_wake_block = RcBlock::new(move |_notification: *const NSNotification| {
                did_wake_callback(DetectEvent::SleepStateChanged { value: false });
            });

            let _observer = unsafe {
                let workspace = NSWorkspace::sharedWorkspace();
                let center = workspace.notificationCenter();

                let will_sleep_name = NSString::from_str("NSWorkspaceWillSleepNotification");
                let did_wake_name = NSString::from_str("NSWorkspaceDidWakeNotification");

                let will_sleep_observer: Retained<NSObject> = msg_send![
                    &*center,
                    addObserverForName: &*will_sleep_name,
                    object: std::ptr::null::<NSObject>(),
                    queue: std::ptr::null::<NSObject>(),
                    usingBlock: &*will_sleep_block
                ];

                let did_wake_observer: Retained<NSObject> = msg_send![
                    &*center,
                    addObserverForName: &*did_wake_name,
                    object: std::ptr::null::<NSObject>(),
                    queue: std::ptr::null::<NSObject>(),
                    usingBlock: &*did_wake_block
                ];

                SleepObserver {
                    center,
                    will_sleep_observer,
                    did_wake_observer,
                }
            };

            let _ = shutdown_rx.recv();
        }));
    }

    fn stop(&mut self) {
        if self.thread_handle.is_none() {
            return;
        }

        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }

        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}
