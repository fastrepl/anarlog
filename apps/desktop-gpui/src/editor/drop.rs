//! Drag and drop between the timeline, notes and chat: shipping's session drag
//! (`chat/context/session-drag.ts`) and its file drops (`packages/editor/src/plugins/
//! file-handler.ts`, `session/components/note-input/file-handler.ts`).

use std::path::{Path, PathBuf};

use desktop_runtime::SessionId;
use gpui::{Context, SharedString, Window, div, prelude::*, px};

use crate::ui::theme::theme;

/// A timeline note being dragged: chat adds it as context, the editor mentions it.
#[derive(Clone, Debug)]
pub struct SessionDrag {
    pub id: SessionId,
    pub title: SharedString,
}

impl SessionDrag {
    /// Shipping's `writeSessionContextDragData` labels an untitled note "Untitled".
    pub fn new(id: SessionId, title: &str) -> Self {
        let title = title.trim();
        Self {
            id,
            title: if title.is_empty() { "Untitled" } else { title }
                .to_owned()
                .into(),
        }
    }
}

/// The ghost under the pointer, like the browser's drag image of the timeline item.
impl Render for SessionDrag {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        div()
            .max_w(px(240.))
            .px_3()
            .py_2()
            .rounded(px(8.))
            .bg(colors.accent)
            .text_sm()
            .text_color(colors.foreground)
            .opacity(0.75)
            .truncate()
            .child(self.title.clone())
    }
}

/// Shipping's `AUDIO_EXTENSIONS` plus `qta`, the types a note accepts as its recording.
const AUDIO: [&str; 9] = [
    "wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac", "qta",
];

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| AUDIO.contains(&extension.to_ascii_lowercase().as_str()))
}

/// The note itself takes a single dropped audio file as its recording, so the editor and
/// the chat composer leave such a drop alone.
pub fn note_audio_drop(paths: &[PathBuf]) -> bool {
    matches!(paths, [path] if is_audio(path))
}

/// Shipping's `getAudioDrop`: the first audio file belongs to the note's recording; every
/// other file is attached where it was dropped.
pub fn attachable(paths: &[PathBuf]) -> Vec<PathBuf> {
    let audio = paths.iter().position(|path| is_audio(path));
    paths
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != audio)
        .map(|(_, path)| path.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_drops_follow_shipping_types_and_leave_the_rest_to_attach() {
        let paths = |names: &[&str]| names.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert!(note_audio_drop(&paths(&["/tmp/Call.M4A"])));
        assert!(note_audio_drop(&paths(&["/tmp/voice.qta"])));
        assert!(!note_audio_drop(&paths(&["/tmp/photo.png"])));
        assert!(!note_audio_drop(&paths(&["/tmp/a.mp3", "/tmp/b.png"])));
        assert_eq!(
            attachable(&paths(&["/tmp/a.png", "/tmp/b.wav", "/tmp/c.mp3"])),
            paths(&["/tmp/a.png", "/tmp/c.mp3"])
        );
        assert_eq!(
            SessionDrag::new(SessionId("s".into()), "  ").title,
            "Untitled"
        );
    }
}
