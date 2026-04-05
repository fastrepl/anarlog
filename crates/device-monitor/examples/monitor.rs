use std::sync::mpsc;
use std::time::Duration;

use device_monitor::{DeviceMonitor, DeviceSwitchMonitor, DeviceUpdateMonitor};

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_default();

    match arg.as_str() {
        "switch" => {
            let (tx, rx) = mpsc::channel();
            let handle = DeviceSwitchMonitor::spawn(tx);
            recv_loop(&rx, 30);
            handle.stop();
        }
        "update" => {
            let (tx, rx) = mpsc::channel();
            let handle = DeviceUpdateMonitor::spawn(tx);
            recv_loop(&rx, 30);
            handle.stop();
        }
        _ => {
            let (tx, rx) = mpsc::channel();
            let handle = DeviceMonitor::spawn(tx);
            recv_loop(&rx, 30);
            handle.stop();
        }
    }
}

fn recv_loop<T: std::fmt::Debug>(rx: &mpsc::Receiver<T>, secs: u64) {
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => println!("{:?}", event),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}
