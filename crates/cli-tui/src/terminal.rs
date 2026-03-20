use std::sync::Once;

static PANIC_HOOK_INSTALLED: Once = Once::new();

pub struct TerminalGuard {
    terminal: ratatui::DefaultTerminal,
    inline: bool,
}

impl TerminalGuard {
    pub fn new() -> Self {
        install_panic_hook();
        Self {
            terminal: ratatui::init(),
            inline: false,
        }
    }

    pub fn new_inline(height: u16) -> std::io::Result<Self> {
        install_panic_hook();
        crossterm::terminal::enable_raw_mode()?;
        let backend = ratatui::backend::CrosstermBackend::new(std::io::stdout());
        let terminal = ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Inline(height),
            },
        )?;
        Ok(Self {
            terminal,
            inline: true,
        })
    }

    pub fn terminal_mut(&mut self) -> &mut ratatui::DefaultTerminal {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if self.inline {
            let _ = crossterm::terminal::disable_raw_mode();
            let _ = crossterm::execute!(std::io::stdout(), crossterm::cursor::Show);
        } else {
            let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::SetTitle(""));
            ratatui::restore();
        }
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
