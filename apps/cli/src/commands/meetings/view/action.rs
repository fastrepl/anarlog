use crossterm::event::KeyEvent;
use hypr_db_app::{MeetingRow, NoteRow};
use hypr_transcript::Segment;

pub(crate) enum Action {
    Key(KeyEvent),
    Paste(String),
    Loaded {
        meeting: MeetingRow,
        segments: Vec<Segment>,
        memo: Option<NoteRow>,
    },
    LoadError(String),
    Saved,
    SaveError(String),
}
