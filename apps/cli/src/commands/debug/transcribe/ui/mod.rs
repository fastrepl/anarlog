pub mod shell;

pub use super::tracing::TracingCapture;

use super::app::App;

pub(crate) fn draw(frame: &mut ratatui::Frame, app: &mut App) {
    let width = frame.area().width.saturating_sub(4) as usize;
    let view = app.transcript_view(width);
    app.shell_mut().draw(
        frame,
        view.title,
        view.lines,
        &view.placeholder,
        view.border_style,
    );
}
