use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use shortcut_macos::{EventTap, HotKey, HotKeyProcessor, Modifier, Modifiers, Output, TapEvent};

fn main() {
    eprintln!(
        "[manual] Requires Accessibility + Input Monitoring. Grant in System Settings if the tap fails."
    );

    let hotkey = HotKey::modifier_only(Modifiers::from([Modifier::Option]));
    let processor = Arc::new(Mutex::new(HotKeyProcessor::new(hotkey)));
    let p = processor.clone();

    let _tap = EventTap::start(move |event| {
        let mut p = p.lock().unwrap();
        let out = match event {
            TapEvent::Key(k) => p.process_key(k),
            TapEvent::MouseClick => p.process_mouse_click(),
        };
        if let Some(o) = out {
            let label = match o {
                Output::StartRecording => "▶ start",
                Output::StopRecording => "■ stop",
                Output::Cancel => "✕ cancel",
                Output::Discard => "… discard",
            };
            println!("{label}");
        }
    })
    .expect("start EventTap");

    println!("[manual] Hold Option to record. Ctrl-C to exit.");
    loop {
        thread::sleep(Duration::from_secs(60));
    }
}
