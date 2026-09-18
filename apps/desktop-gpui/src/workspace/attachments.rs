//! `useNoteFileHandlerConfig` for pasted (and dropped) files: each file is
//! saved as a session attachment and inserted as an `image` or
//! `fileAttachment` node; failures surface as `sonnerToast.error`.

use std::path::PathBuf;

use gpui::{AppContext as _, Context, Entity};

use super::Workspace;
use super::toast::FlashVariant;
use crate::db::attachments::{file_attachment_node, image_node, is_image_mime};
use crate::editor::{BodyEditor, PastedFile};

/// `AUDIO_TRANSFER_EXTENSIONS`: the audio files a drop hands to the upload
/// flow instead of attaching.
const AUDIO_TRANSFER_EXTENSIONS: [&str; 9] = [
    "wav", "mp3", "ogg", "mp4", "m4a", "flac", "webm", "aac", "qta",
];

fn file_extension(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_default()
}

/// `File.type` for a dropped path: the browser's extension-based guess.
fn mime_type_for(path: &std::path::Path) -> String {
    mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("")
        .to_string()
}

impl Workspace {
    /// `handleDrop` with `onDrop`: the first audio file (`getAudioDrop`) runs
    /// the audio import, the rest are read and attached.
    pub(super) fn drop_files(
        &mut self,
        session_id: String,
        editor: Entity<BodyEditor>,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let audio_index = paths.iter().position(|path| {
            AUDIO_TRANSFER_EXTENSIONS.contains(&file_extension(path).as_str())
                || mime_type_for(path).starts_with("audio/")
        });
        let mut remaining = paths;
        if let Some(index) = audio_index {
            let audio = remaining.remove(index);
            self.run_audio_import(session_id.clone(), audio, cx);
        }
        if remaining.is_empty() {
            return;
        }
        cx.spawn(async move |this, cx| {
            let files = cx
                .background_spawn(async move {
                    remaining
                        .into_iter()
                        .map(|path| {
                            let name = path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("file")
                                .to_string();
                            let mime_type = mime_type_for(&path);
                            std::fs::read(&path).map(|bytes| PastedFile {
                                name,
                                mime_type,
                                bytes,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            this.update(cx, |this, cx| {
                let mut ready = Vec::new();
                for file in files {
                    match file {
                        Ok(file) => ready.push(file),
                        Err(error) => {
                            this.flash(FlashVariant::Error, error.to_string(), cx);
                        }
                    }
                }
                if !ready.is_empty() {
                    this.attach_files(session_id, editor, ready, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    /// `handleFiles`: uploads run one after another, each node inserted at
    /// the selection of the moment its upload finishes.
    pub(super) fn attach_files(
        &mut self,
        session_id: String,
        editor: Entity<BodyEditor>,
        files: Vec<PastedFile>,
        cx: &mut Context<Self>,
    ) {
        let store = self.store.clone();
        cx.spawn(async move |this, cx| {
            for file in files {
                let saved = store
                    .save_note_attachment(
                        session_id.clone(),
                        file.name.clone(),
                        file.mime_type.clone(),
                        file.bytes,
                    )
                    .await
                    .map_err(anyhow::Error::from)
                    .and_then(|result| result);
                match saved {
                    Ok(saved) => {
                        let node = if is_image_mime(&file.mime_type) {
                            image_node(&saved)
                        } else {
                            file_attachment_node(&saved, &file.name, &file.mime_type)
                        };
                        editor
                            .update(cx, |editor, cx| editor.insert_attachment(node, cx))
                            .ok();
                    }
                    Err(error) => {
                        tracing::error!(%error, "Failed to upload file");
                        // `handleFileUploadError`
                        let message = error.to_string();
                        this.update(cx, |this, cx| {
                            this.flash(
                                FlashVariant::Error,
                                if message.is_empty() {
                                    "Could not add this attachment.".to_string()
                                } else {
                                    message
                                },
                                cx,
                            )
                        })
                        .ok();
                    }
                }
            }
        })
        .detach();
    }
}
