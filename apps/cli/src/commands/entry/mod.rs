use crate::{
    event::{EventHandler, TuiEvent},
    frame::FrameRequester,
    terminal::TerminalGuard,
};

mod app;
mod ui;

pub use app::EntryAction;

use app::EntryApp;

pub struct Args {
    pub status_message: Option<String>,
}

pub async fn run(args: Args) -> EntryAction {
    let mut terminal = TerminalGuard::new();
    let (draw_tx, draw_rx) = tokio::sync::broadcast::channel(16);
    let frame_requester = FrameRequester::new(draw_tx);
    let mut app = EntryApp::new(frame_requester.clone(), args.status_message);
    let mut events = EventHandler::new(draw_rx);
    events.resume_events();

    frame_requester.schedule_frame();

    loop {
        tokio::select! {
            Some(tui_event) = events.next() => {
                match tui_event {
                    TuiEvent::Key(key) => app.handle_key(key),
                    TuiEvent::Paste(pasted) => app.handle_paste(pasted),
                    TuiEvent::Draw => {
                        terminal
                            .terminal_mut()
                            .draw(|frame| ui::draw(frame, &mut app))
                            .ok();
                    }
                }
            }
            else => break,
        }

        if app.should_quit {
            break;
        }
    }

    events.pause_events();

    app.action().unwrap_or(EntryAction::Quit)
}
