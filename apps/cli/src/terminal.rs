use std::sync::Once;

static PANIC_HOOK_INSTALLED: Once = Once::new();

pub struct TerminalGuard {
    terminal: ratatui::DefaultTerminal,
}

impl TerminalGuard {
    pub fn new() -> Self {
        install_panic_hook();
        Self {
            terminal: ratatui::init(),
        }
    }

    pub fn terminal_mut(&mut self) -> &mut ratatui::DefaultTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        ratatui::restore();
    }
}

fn install_panic_hook() {
    PANIC_HOOK_INSTALLED.call_once(|| {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            ratatui::restore();
            original(info);
        }));
    });
}
