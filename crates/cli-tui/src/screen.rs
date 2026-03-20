use std::convert::Infallible;
use std::future;
use std::time::Duration;

use ratatui::Frame;
use tokio::sync::mpsc;

use crate::{EventHandler, FrameRequester, TerminalGuard, TuiEvent};

const DEFAULT_IDLE_FRAME: Duration = Duration::from_secs(1);

pub enum ScreenControl<T> {
    Continue,
    Exit(T),
}

pub struct ScreenContext {
    frame_requester: FrameRequester,
}

impl ScreenContext {
    fn new(frame_requester: FrameRequester) -> Self {
        Self { frame_requester }
    }

    pub fn frame_requester(&self) -> &FrameRequester {
        &self.frame_requester
    }
}

pub fn terminal_title(subtitle: Option<&str>) -> String {
    match subtitle {
        Some(s) => format!("Char | {s}"),
        None => "Char".to_string(),
    }
}

pub trait Screen {
    type ExternalEvent;
    type Output;

    fn on_tui_event(
        &mut self,
        event: TuiEvent,
        cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output>;

    fn on_external_event(
        &mut self,
        event: Self::ExternalEvent,
        cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output>;

    fn draw(&mut self, frame: &mut Frame);

    fn on_resize(&mut self) {}

    fn title(&self) -> String {
        String::new()
    }

    fn next_frame_delay(&self) -> Duration {
        DEFAULT_IDLE_FRAME
    }
}

pub async fn run_screen<S>(
    screen: S,
    external_rx: Option<mpsc::UnboundedReceiver<S::ExternalEvent>>,
) -> std::io::Result<S::Output>
where
    S: Screen,
{
    let terminal = TerminalGuard::new();
    run_screen_with(screen, terminal, true, external_rx).await
}

pub async fn run_screen_inline<S>(
    screen: S,
    height: u16,
    external_rx: Option<mpsc::UnboundedReceiver<S::ExternalEvent>>,
) -> std::io::Result<S::Output>
where
    S: Screen,
{
    let terminal = TerminalGuard::new_inline(height)?;
    run_screen_with(screen, terminal, false, external_rx).await
}

async fn run_screen_with<S>(
    mut screen: S,
    mut terminal: TerminalGuard,
    set_title: bool,
    external_rx: Option<mpsc::UnboundedReceiver<S::ExternalEvent>>,
) -> std::io::Result<S::Output>
where
    S: Screen,
{
    let (draw_tx, draw_rx) = tokio::sync::broadcast::channel(16);
    let frame_requester = FrameRequester::new(draw_tx);
    let mut cx = ScreenContext::new(frame_requester.clone());
    let mut events = EventHandler::new(draw_rx);
    let mut external_rx = external_rx;
    let mut events_open = true;
    let mut external_open = external_rx.is_some();

    events.resume_events();
    frame_requester.schedule_frame();

    let output = loop {
        tokio::select! {
            tui_event = events.next(), if events_open => {
                match tui_event {
                    Some(TuiEvent::Resize) => {
                        screen.on_resize();
                        if set_title {
                            let _ = crossterm::execute!(
                                std::io::stdout(),
                                crossterm::terminal::SetTitle(screen.title()),
                            );
                        }
                        terminal
                            .terminal_mut()
                            .draw(|frame| screen.draw(frame))?;
                        frame_requester.schedule_frame_in(screen.next_frame_delay());
                    }
                    Some(TuiEvent::Draw) => {
                        if set_title {
                            let _ = crossterm::execute!(
                                std::io::stdout(),
                                crossterm::terminal::SetTitle(screen.title()),
                            );
                        }
                        terminal
                            .terminal_mut()
                            .draw(|frame| screen.draw(frame))?;
                        frame_requester.schedule_frame_in(screen.next_frame_delay());
                    }
                    Some(other) => {
                        match screen.on_tui_event(other, &mut cx) {
                            ScreenControl::Continue => frame_requester.schedule_frame(),
                            ScreenControl::Exit(output) => break output,
                        }
                    }
                    None => {
                        events_open = false;
                    }
                }
            }
            external = recv_external_event(&mut external_rx), if external_open => {
                match external {
                    Some(event) => {
                        match screen.on_external_event(event, &mut cx) {
                            ScreenControl::Continue => frame_requester.schedule_frame(),
                            ScreenControl::Exit(output) => break output,
                        }
                    }
                    None => {
                        external_open = false;
                    }
                }
            }
            else => {
                return Err(std::io::Error::other("screen event loop closed"));
            },
        }
    };

    events.pause_events();
    drop(terminal);

    Ok(output)
}

async fn recv_external_event<T>(external_rx: &mut Option<mpsc::UnboundedReceiver<T>>) -> Option<T> {
    let Some(rx) = external_rx.as_mut() else {
        future::pending().await
    };
    rx.recv().await
}

impl Screen for () {
    type ExternalEvent = Infallible;
    type Output = ();

    fn on_tui_event(
        &mut self,
        _event: TuiEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        ScreenControl::Continue
    }

    fn on_external_event(
        &mut self,
        _event: Self::ExternalEvent,
        _cx: &mut ScreenContext,
    ) -> ScreenControl<Self::Output> {
        ScreenControl::Continue
    }

    fn draw(&mut self, _frame: &mut Frame) {}
}
