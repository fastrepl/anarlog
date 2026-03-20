pub(crate) mod list;
mod notepad;
pub(crate) mod status_bar;
mod transcript;

pub(crate) use notepad::draw_notepad;
pub(crate) use status_bar::draw_status_bar;
pub(crate) use transcript::draw_transcript;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    Insert,
    Command,
}
