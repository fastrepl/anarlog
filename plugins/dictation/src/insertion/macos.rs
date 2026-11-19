//! The captured field through Accessibility (`crates/dictation-ui-macos`), and the default
//! output's volume through CoreAudio.

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

use anlg_dictation_ui_macos as ui;
use dictation_core::context::{WindowFacts, is_browser};

use super::{
    edit::{FieldPort, FieldRead, Paste, Span, WINDOW_UTF16, Write},
    volume::{Output, SystemVolume},
};

/// How long a sent paste waits for the app to read the text before it is withdrawn.
const PASTE_READ_WAIT: Duration = Duration::from_millis(2_000);
const PASTE_POLL: Duration = Duration::from_millis(15);
const SETTLE: Duration = Duration::from_millis(40);

pub async fn capture() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(ui::capture_target)
        .await
        .map_err(|error| error.to_string())?
}

pub struct Field {
    target: String,
    /// A paste the app read, kept on the clipboard until the app is done with it.
    delivered: Cell<Option<isize>>,
}

pub fn field(target: &str) -> Option<Field> {
    Some(Field {
        target: target.to_owned(),
        delivered: Cell::new(None),
    })
}

impl Field {
    /// The app read paste `id`; it stays on the clipboard until `paste_consumed`.
    fn hold(&self, id: isize) -> Paste {
        self.delivered.set(Some(id));
        Paste::Delivered
    }
}

impl FieldPort for Field {
    fn focused(&self) -> bool {
        ui::target_focused(&self.target)
    }

    fn read(&self) -> Option<FieldRead> {
        let text = ui::target_read(&self.target, WINDOW_UTF16)?;
        let selected = text.selected.encode_utf16().count();
        Some(FieldRead {
            length: text.length,
            selection: Span {
                start: text.start,
                len: selected,
            },
            before: text.before,
            selected: text.selected,
            after: text.after,
        })
    }

    fn select(&self, span: Span) -> bool {
        ui::target_select(&self.target, span.start, span.len)
    }

    fn write(&self, text: &str) -> Write {
        match ui::target_write(&self.target, text) {
            ui::TargetWrite::Done => Write::Accepted,
            ui::TargetWrite::Refused => Write::Rejected,
            ui::TargetWrite::Unconfirmed => Write::Unconfirmed,
        }
    }

    fn paste(&self, text: &str) -> Paste {
        let Some(id) = ui::target_paste(&self.target, text) else {
            return Paste::NotSent;
        };
        let deadline = Instant::now() + PASTE_READ_WAIT;
        while Instant::now() < deadline {
            match ui::paste_status(id) {
                ui::PasteState::Read => return self.hold(id),
                ui::PasteState::Gone => break,
                ui::PasteState::Pending => std::thread::sleep(PASTE_POLL),
            }
        }
        match ui::paste_revoke(id) {
            ui::PasteState::Read => self.hold(id),
            ui::PasteState::Pending | ui::PasteState::Gone => Paste::Revoked,
        }
    }

    fn paste_consumed(&self) {
        if let Some(id) = self.delivered.take() {
            ui::paste_finish(id);
        }
    }

    fn settle(&self) {
        std::thread::sleep(SETTLE);
    }
}

pub fn facts(target: &str) -> Option<WindowFacts> {
    let facts = ui::target_facts(target)?;
    let browser = facts.bundle_id.as_deref().is_some_and(is_browser);
    Some(WindowFacts {
        bundle_id: facts.bundle_id,
        app_name: facts.app_name,
        title: facts.title,
        secure_field: Some(facts.secure),
        // Accessibility can't tell a private browser window from another, so a browser's is
        // unknown, which denies the read; other apps have no private windows.
        private_window: (!browser).then_some(false),
        browser_host: None,
    })
}

pub fn window_text(target: &str, max_chars: usize) -> Option<String> {
    ui::target_window_text(target, max_chars)
}

pub fn release(target: &str) {
    ui::target_release(target);
}

struct CoreAudio;

impl SystemVolume for CoreAudio {
    fn output(&self) -> Option<Output> {
        ui::output_level().map(|level| Output {
            device: level.device,
            profile: level.profile,
        })
    }

    fn get(&self) -> Option<f32> {
        ui::output_level()?.volume
    }

    fn set(&self, device: &str, volume: f32) -> bool {
        ui::set_output_level(device, volume)
    }
}

pub fn system_volume() -> Option<Box<dyn SystemVolume>> {
    Some(Box::new(CoreAudio))
}
