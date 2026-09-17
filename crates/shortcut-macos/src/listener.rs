use std::sync::{Arc, Mutex};

use crate::{
    hotkey::HotKey,
    processor::{HotKeyProcessor, Options, Output},
    tap::{EventTap, TapError, TapEvent},
};

pub struct Listener {
    _tap: EventTap,
}

impl Listener {
    pub fn start<F>(hotkey: HotKey, options: Options, callback: F) -> Result<Self, TapError>
    where
        F: Fn(Output) + Send + Sync + 'static,
    {
        let processor = Arc::new(Mutex::new({
            let mut p = HotKeyProcessor::new(hotkey);
            p.set_options(options);
            p
        }));

        let tap = EventTap::start_filtered(move |event| {
            let (out, consume) = {
                let mut p = processor.lock().unwrap_or_else(|e| e.into_inner());
                let out = match event {
                    TapEvent::Key(k) => p.process_key(k),
                    TapEvent::MouseClick => p.process_mouse_click(),
                };
                let consume = hotkey.is_modifier_only()
                    && matches!(event, TapEvent::Key(k) if k.key.is_none())
                    && out.is_some();
                (out, consume)
            };
            if let Some(out) = out {
                callback(out);
            }
            consume
        })?;

        Ok(Self { _tap: tap })
    }
}
