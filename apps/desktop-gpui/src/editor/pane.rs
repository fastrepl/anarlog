mod conflict;

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use desktop_runtime::{AttachmentId, CancellationToken, DocumentSnapshot, HumanId, ServiceError};
use futures::{StreamExt, channel::oneshot};
use gpui::{
    App, ClipboardItem, Context, ElementInputHandler, Entity, EventEmitter, FocusHandle, Focusable,
    ListAlignment, ListOffset, ListState, MouseButton, Pixels, Point, SharedString, Window, canvas,
    div, list, prelude::*, px,
};

use super::{
    clipboard,
    document::{Document, EditResult},
    find::FindOptions,
    menu::{
        BlockCommand, EditorRequest, MentionCandidate, MentionMenu, MentionResults, MentionTarget,
        SlashMenu,
    },
    model::{EditorModel, Selection},
    persistence::{Flush, SaveEvent, SaveJournal},
    surface::{self, ProjectedBlock, VisibleLayout},
};
use crate::ui::a11y::Accessible;
use crate::{
    contracts::{EditorEvent, EditorInit, LaneContext},
    ui::{caret::Caret, theme::theme},
};

pub struct EditorPane {
    pub context: LaneContext,
    pub init: EditorInit,
    pub(super) model: Option<EditorModel>,
    pub(super) focus: FocusHandle,
    pub(super) caret: Entity<Caret>,
    pub(super) journal: Option<SaveJournal>,
    dirty: bool,
    /// Shipping autosaves silently; the retry control only appears after a failed save.
    save_failed: bool,
    pub(super) message: String,
    list: ListState,
    list_count: usize,
    projection: HashMap<u64, Arc<ProjectedBlock>>,
    projecting: HashSet<u64>,
    pub(super) layouts: HashMap<u64, VisibleLayout>,
    pub(super) slash: Option<SlashMenu>,
    pub(super) slash_dismissed: Option<usize>,
    placeholder: Option<SharedString>,
    /// The note title shown as the first H1, as shipping's summary editor does.
    title: Option<SharedString>,
    pub(super) mention: Option<MentionMenu>,
    mention_results: MentionResults,
    mention_enabled: bool,
    external_mention_provider: bool,
    mention_cancel: Option<CancellationToken>,
    focused: bool,
    read_only: bool,
    dragging: bool,
    drag_position: Option<Point<Pixels>>,
    drag_scroll: Option<gpui::Task<()>>,
    pub(super) viewport: Option<gpui::Bounds<Pixels>>,
    pending_reveal: Option<usize>,
    clipboard_generation: u64,
    external_generation: u64,
    external: Option<DocumentSnapshot>,
    watch_cancel: CancellationToken,
    pub(super) link_input: Option<String>,
    attachment_service: Option<super::AttachmentService>,
    /// Rendered after the last block, inside the same scroll (shipping's memo extras).
    footer: Option<gpui::AnyView>,
    attachment_previews: HashMap<String, Result<super::AttachmentPreview, String>>,
    attachment_loading: HashSet<String>,
    pending_attachment: Option<super::AttachmentPreview>,
    attachment_importing: bool,
    pub(super) find: Option<Found>,
    /// A find match to scroll to the viewport's center once laid out (shipping's
    /// `scrollIntoView({ block: "center" })`).
    find_center: Option<usize>,
    /// Shipping's `dropCursor`: where a dragged note or file would land.
    pub(super) drop_cursor: Option<usize>,
    /// The dragged files are one audio file, which the note takes as its recording instead.
    drop_declined: bool,
    /// The image under the pointer and the clicked (node-selected) image, by node id.
    hovered_image: Option<u64>,
    pub(super) selected_image: Option<(usize, u64)>,
    /// Each painted image's width, where a resize starts from.
    image_widths: HashMap<u64, Pixels>,
    pub(super) resize: Option<ImageResize>,
}

/// An image being resized from one of its handles (shipping's `handleResizeStart`).
pub(super) struct ImageResize {
    pub(super) id: u64,
    pub(super) start: usize,
    pub(super) left: bool,
    pub(super) origin: Pixels,
    pub(super) initial: Pixels,
    pub(super) max: Pixels,
    /// The drag's width in pixels; it becomes a percentage of the column on release.
    pub(super) width: Pixels,
}

/// Shipping's `MIN_IMAGE_WIDTH`, `MAX_IMAGE_WIDTH` and `DEFAULT_IMAGE_WIDTH` (percent).
const IMAGE_WIDTH: (f64, f64, f64) = (15., 100., 80.);

/// Shipping's `clampImageWidth`.
fn image_width(node: &super::document::NodeRef) -> f64 {
    node.attr("editorWidth")
        .and_then(serde_json::Value::as_f64)
        .filter(|width| width.is_finite())
        .map_or(IMAGE_WIDTH.2, |width| {
            width.round().clamp(IMAGE_WIDTH.0, IMAGE_WIDTH.1)
        })
}

/// The note's find query and its matches in the current document.
pub(super) struct Found {
    query: String,
    options: FindOptions,
    active: usize,
    pub(super) matches: Vec<std::ops::Range<usize>>,
    /// The document root the matches were computed for.
    root: usize,
}

impl EditorPane {
    pub fn new(
        context: LaneContext,
        init: EditorInit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(context, init, None, window, cx)
    }

    /// A summary editor: `title` leads the document as its first H1 (added for display
    /// only; stored bytes change on the first edit, as in shipping).
    pub fn titled(
        context: LaneContext,
        init: EditorInit,
        title: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::create(context, init, Some(title), window, cx)
    }

    fn create(
        context: LaneContext,
        init: EditorInit,
        title: Option<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        cx.on_focus(&focus, window, |this, _, _| this.focused = true)
            .detach();
        cx.on_blur(&focus, window, |this, _, cx| {
            this.focused = false;
            this.dismiss_mention();
            if !this.dirty
                && !this.model.as_ref().is_some_and(EditorModel::composing)
                && let Some(snapshot) = this.external.take()
            {
                this.load_clean_revision(snapshot, cx);
            }
        })
        .detach();
        focus.focus(window, cx);
        let caret = Caret::new(&focus, window, cx);
        let watch_cancel = CancellationToken::new();
        let (sender, mut receiver) = futures::channel::mpsc::channel(1);
        cx.background_executor()
            .spawn(super::services::watch(
                context.runtime.clone(),
                init.document.id.clone(),
                watch_cancel.clone(),
                sender,
            ))
            .detach();
        // The window's focus decides whether a remote revision waits: the `focused` flag is
        // not updated while the editor is off screen.
        cx.spawn_in(window, async move |entity, cx| {
            while let Some(result) = receiver.next().await {
                if entity
                    .update_in(cx, |this, window, cx| {
                        match result {
                            Ok(Some(snapshot)) => {
                                this.apply_external_revision(snapshot, window, cx)
                            }
                            // An unsaved memo has no row until its first save creates it.
                            Ok(None) if this.init.document.is_unsaved() => {}
                            Ok(None) => {
                                this.message =
                                    "This document was removed remotely; local content retained."
                                        .into();
                                this.set_read_only(true, cx);
                            }
                            Err(error) => this.message = format!("Document watch stopped: {error}"),
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        let runtime = context.runtime.clone();
        let snapshot = init.document.clone();
        let display_title = title.clone();
        let load = cx.background_executor().spawn(async move {
            let body: Arc<str> = match snapshot.body_format.as_ref() {
                "prosemirror_json" => snapshot.body.clone(),
                // Shipping decodes imported Markdown for display; the first edit saves
                // ProseMirror JSON in its place, as shipping does.
                "markdown" => anlg_tiptap::md_to_tiptap_json(&snapshot.body)?
                    .to_string()
                    .into(),
                format => {
                    return Err(format!(
                        "Unsupported stored format: {format}. Original bytes retained read-only."
                    ));
                }
            };
            let document = Document::parse(with_title(body, display_title.as_deref()))?;
            let (journal, events) = SaveJournal::start(runtime, snapshot)
                .await
                .map_err(|error| error.to_string())?;
            Ok((document, journal, events))
        });
        cx.spawn(async move |entity, cx| {
            let loaded = load.await;
            let mut events = None;
            let _ = entity.update(cx, |this, cx| {
                match loaded {
                    Ok((document, journal, receiver)) => {
                        this.list_count = document.root.children.render_blocks();
                        this.list.reset(this.list_rows());
                        let mut model = EditorModel::new(document);
                        model.read_only = this.read_only;
                        this.model = Some(model);
                        this.journal = Some(journal);
                        this.message.clear();
                        events = Some(receiver);
                    }
                    Err(error) => this.message = error,
                }
                cx.notify();
            });
            if let Some(mut events) = events {
                while let Some(event) = events.next().await {
                    if entity.update(cx, |this, cx| {
                        match event {
                            SaveEvent::Saved { revision, snapshot } => {
                                if this.external.as_ref().is_some_and(|external| external.id == snapshot.id && external.updated_at == snapshot.updated_at) {
                                    this.external = None;
                                }
                                this.init.document = snapshot.clone();
                                if this.model.as_ref().is_some_and(|model| model.revision == revision && !model.composing()) {
                                    this.dirty = false;
                                    cx.emit(EditorEvent::Dirty { session_id: this.init.session_id.clone(), dirty: false });
                                }
                                this.message.clear();
                                this.save_failed = false;
                                cx.emit(EditorEvent::Saved(snapshot));
                            }
                            SaveEvent::Failed(error) => {
                                // A conflict is explained by the note's conflict banner once the
                                // other revision loads.
                                if matches!(error, ServiceError::Conflict) {
                                    this.message.clear();
                                    this.fetch_conflict(cx);
                                } else {
                                    this.message = format!("Unsaved changes: {error} Draft retained; retry or resolve the external revision.");
                                }
                                this.save_failed = true;
                                cx.emit(EditorEvent::SaveFailed { session_id: this.init.session_id.clone(), error });
                            }
                        }
                        cx.notify();
                    }).is_err() { break; }
                }
            }
        }).detach();
        Self {
            context,
            init,
            focus,
            model: None,
            journal: None,
            dirty: false,
            save_failed: false,
            message: String::new(),
            list: ListState::new(0, ListAlignment::Top, px(400.)),
            list_count: 0,
            projection: HashMap::new(),
            projecting: HashSet::new(),
            layouts: HashMap::new(),
            slash: None,
            slash_dismissed: None,
            placeholder: Some("Start writing...".into()),
            title,
            mention: None,
            mention_results: MentionResults::default(),
            mention_enabled: true,
            external_mention_provider: false,
            mention_cancel: None,
            focused: true,
            caret,
            read_only: false,
            dragging: false,
            drag_position: None,
            drag_scroll: None,
            viewport: None,
            pending_reveal: None,
            clipboard_generation: 0,
            external_generation: 0,
            external: None,
            watch_cancel,
            link_input: None,
            attachment_service: None,
            footer: None,
            attachment_previews: HashMap::new(),
            attachment_loading: HashSet::new(),
            pending_attachment: None,
            attachment_importing: false,
            find: None,
            find_center: None,
            drop_cursor: None,
            drop_declined: false,
            hovered_image: None,
            selected_image: None,
            image_widths: HashMap::new(),
            resize: None,
        }
    }

    fn drag_over(
        &mut self,
        position: Point<Pixels>,
        bounds: gpui::Bounds<Pixels>,
        declined: bool,
        cx: &mut Context<Self>,
    ) {
        let cursor = (bounds.contains(&position) && !declined && !self.read_only)
            .then(|| self.hit(position))
            .flatten();
        if self.drop_cursor != cursor || self.drop_declined != declined {
            self.drop_cursor = cursor;
            self.drop_declined = declined;
            cx.notify();
        }
    }

    /// Shipping's `sessionMentionDrop`: a timeline note dropped into the note becomes a
    /// session mention followed by a space, at the drop point.
    fn drop_session(
        &mut self,
        drag: &super::drop::SessionDrag,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = self
            .drop_cursor
            .take()
            .or_else(|| self.hit(window.mouse_position()));
        let Some(model) = &mut self.model else {
            return;
        };
        let attrs = serde_json::Map::from_iter([
            ("id".into(), serde_json::json!(drag.id.0.as_ref())),
            ("type".into(), serde_json::json!("session")),
            ("label".into(), serde_json::json!(drag.title.as_ref())),
        ]);
        let before = model.revision;
        let result = model.transaction(|model| {
            if let Some(position) =
                position.filter(|position| model.document.resolve(*position).is_ok())
            {
                model.select(Selection::caret(position));
            }
            model.insert_inline_atom("mention-@", attrs)?;
            let head = model.selection.head;
            model.replace(head..head, " ")
        });
        self.edited(before, result, cx);
        self.focus.focus(window, cx);
    }

    /// Shipping's `fileHandlerPlugin` drop: each file is catalogued as an attachment and
    /// inserted at the drop point, images as images and the rest as file cards.
    fn drop_files(
        &mut self,
        paths: &[std::path::PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = self
            .drop_cursor
            .take()
            .or_else(|| self.hit(window.mouse_position()));
        // Shipping's `getAudioDrop`: the first audio file becomes the note's recording.
        if let Some(audio) = paths.iter().find(|path| super::drop::is_audio(path)) {
            let (runtime, session) = (self.context.runtime.clone(), self.init.session_id.clone());
            crate::meeting::upload::Uploads::global(cx).update(cx, |uploads, cx| {
                uploads.drop_audio(&runtime, session, audio.clone(), cx)
            });
        }
        let files = super::drop::attachable(paths);
        if files.is_empty() {
            return;
        }
        let Some(service) = self.attachment_service.clone() else {
            self.message = "Configure the active vault before importing attachments.".into();
            cx.notify();
            return;
        };
        if self.attachment_importing {
            self.message =
                "Wait for the current attachment import before adding another file.".into();
            cx.notify();
            return;
        }
        self.attachment_importing = true;
        let session = self.init.session_id.clone();
        let job = cx.background_executor().spawn(async move {
            let mut imported = Vec::with_capacity(files.len());
            for file in files {
                imported.push(service.import(session.clone(), file).await);
            }
            imported
        });
        cx.spawn(async move |entity, cx| {
            let imported = job.await;
            let _ = entity.update(cx, |this, cx| {
                this.attachment_importing = false;
                let mut error = None;
                let previews: Vec<_> = imported
                    .into_iter()
                    .filter_map(|result| result.map_err(|failure| error = Some(failure)).ok())
                    .collect();
                this.insert_attachments(previews, position, cx);
                if let Some(error) = error {
                    this.message = error;
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn insert_attachments(
        &mut self,
        previews: Vec<super::AttachmentPreview>,
        position: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if previews.is_empty() {
            return;
        }
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let result = model.transaction(|model| {
            if let Some(position) =
                position.filter(|position| model.document.resolve(*position).is_ok())
            {
                model.select(Selection::caret(position));
            }
            previews.iter().try_for_each(|preview| {
                let (kind, attrs) = attachment_node(preview);
                model.insert_block_atom(kind, attrs)
            })
        });
        for preview in previews {
            self.cache_attachment_preview(preview.id.clone(), Ok(preview));
        }
        self.edited(before, result, cx);
    }

    fn paint_drop_cursor(&self, window: &mut Window) {
        if let Some(bounds) = self.drop_cursor.and_then(|position| {
            self.layouts
                .values()
                .find_map(|layout| layout.bounds_for(position..position))
        }) {
            // prosemirror-dropcursor's default: a 1px black bar.
            window.paint_quad(gpui::fill(
                gpui::Bounds::new(bounds.origin, gpui::size(px(1.), bounds.size.height)),
                gpui::black(),
            ));
        }
    }

    pub(super) fn resize_move(&mut self, x: Pixels, cx: &mut Context<Self>) {
        if let Some(resize) = &mut self.resize {
            let delta = (x - resize.origin) * if resize.left { -1. } else { 1. };
            resize.width = (resize.initial + delta).max(px(120.)).min(resize.max);
            cx.notify();
        }
    }

    /// On release the drag's width is stored as a percentage of the column, like shipping.
    pub(super) fn resize_commit(&mut self, cx: &mut Context<Self>) {
        let (Some(resize), Some(model)) = (self.resize.take(), &mut self.model) else {
            return;
        };
        let percent = (f64::from(f32::from(resize.width) / f32::from(resize.max)) * 100.)
            .round()
            .clamp(IMAGE_WIDTH.0, IMAGE_WIDTH.1);
        let before = model.revision;
        let result = model
            .atom_path(resize.start, resize.id)
            .ok_or_else(|| "Image moved; resize again".to_owned())
            .and_then(|path| {
                model.update_node_attrs(
                    &path,
                    serde_json::Map::from_iter([(
                        "editorWidth".into(),
                        serde_json::json!(percent),
                    )]),
                )
            });
        let index = surface::block_index(&model.document, resize.start);
        self.edited(before, result, cx);
        if let Some(index) = index.filter(|index| *index < self.list_count) {
            self.list.splice(index..index + 1, 1);
        }
    }

    /// Shipping's `setSearch`: highlights every match and scrolls to the first.
    pub fn find(&mut self, query: &str, options: FindOptions, cx: &mut Context<Self>) {
        self.find = (!query.trim().is_empty()).then(|| Found {
            query: query.into(),
            options,
            active: 0,
            matches: Vec::new(),
            root: 0,
        });
        self.refresh_find();
        self.reveal_found();
        cx.notify();
    }

    /// The current match and how many there are, after any edit since the last search.
    pub fn find_status(&mut self) -> (usize, usize) {
        self.refresh_find();
        self.find
            .as_ref()
            .map_or((0, 0), |find| (find.active, find.matches.len()))
    }

    pub fn find_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.refresh_find();
        if let Some(find) = &mut self.find
            && !find.matches.is_empty()
        {
            let len = find.matches.len();
            find.active = (find.active + if forward { 1 } else { len - 1 }) % len;
            self.reveal_found();
            cx.notify();
        }
    }

    /// Shipping's `replace`: the current match, or every match in one undoable edit.
    pub fn replace_found(&mut self, replacement: &str, all: bool, cx: &mut Context<Self>) {
        self.refresh_find();
        let (Some(find), Some(model)) = (&self.find, &mut self.model) else {
            return;
        };
        let Some(current) = find.matches.get(find.active).cloned() else {
            return;
        };
        let ranges = if all {
            find.matches.clone()
        } else {
            vec![current.clone()]
        };
        let blocks: std::collections::BTreeSet<_> = ranges
            .iter()
            .filter_map(|range| surface::block_index(&model.document, range.start))
            .collect();
        let before = model.revision;
        let result = model.transaction(|model| {
            ranges
                .iter()
                .rev()
                .try_for_each(|range| model.replace(range.clone(), replacement))
        });
        self.edited(before, result, cx);
        for index in blocks {
            if index < self.list_count {
                self.list.splice(index..index + 1, 1);
            }
        }
        self.refresh_find();
        let resume = current.start + replacement.encode_utf16().count();
        if let Some(find) = &mut self.find {
            find.active = if all {
                0
            } else {
                find.matches
                    .iter()
                    .position(|range| range.start >= resume)
                    .unwrap_or(0)
            };
        }
        if !all {
            self.reveal_found();
        }
        cx.notify();
    }

    fn refresh_find(&mut self) {
        let (Some(find), Some(model)) = (&mut self.find, &self.model) else {
            return;
        };
        let root = Arc::as_ptr(&model.document.root) as usize;
        if find.root != root {
            find.root = root;
            find.matches =
                super::find::document_matches(&model.document, &find.query, find.options);
            find.active = find.active.min(find.matches.len().saturating_sub(1));
        }
    }

    fn reveal_found(&mut self) {
        let Some(position) = self
            .find
            .as_ref()
            .and_then(|find| find.matches.get(find.active))
            .map(|range| range.start)
        else {
            return;
        };
        self.find_center = Some(position);
        if !self
            .layouts
            .values()
            .any(|layout| layout.bounds_for(position..position).is_some())
            && let Some(model) = &self.model
            && let Some(index) = surface::block_index(&model.document, position)
        {
            self.list.scroll_to(ListOffset {
                item_ix: index,
                offset_in_item: px(0.),
            });
        }
    }

    fn finish_find_reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(position), Some(viewport)) = (self.find_center, self.viewport) else {
            return;
        };
        let Some(bounds) = self
            .layouts
            .values()
            .find_map(|layout| layout.bounds_for(position..position))
        else {
            return;
        };
        self.find_center = None;
        let distance = bounds.center().y - viewport.center().y;
        if distance.abs() > px(1.) {
            let entity = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                let _ = entity.update(cx, |this, cx| {
                    this.list.scroll_by(distance);
                    cx.notify();
                });
            });
        }
    }

    /// The empty-block hint; memos say "Start writing..." like shipping, summaries show none.
    pub fn set_placeholder(&mut self, placeholder: Option<SharedString>) {
        self.placeholder = placeholder;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Shipping's `isPristineNoteDoc`: nothing but one empty paragraph.
    pub fn is_pristine(&self) -> bool {
        self.model
            .as_ref()
            .is_some_and(|model| super::title::is_pristine(&model.document.root.children))
    }

    /// Shipping's memo template: each titled section as an H2 over an empty paragraph,
    /// replacing the (pristine) memo in one undoable edit that autosaves.
    pub fn apply_template(&mut self, sections: &[&str], cx: &mut Context<Self>) {
        let Some(body) = super::title::template_body(sections) else {
            return;
        };
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let result = Document::parse(body.to_string().into())
            .and_then(|fragment| model.replace_all(fragment));
        self.edited(before, result, cx);
    }

    /// Shipping's `replaceContent` for text streamed into the document (the pre-meeting
    /// brief): the whole document becomes `body` (TipTap JSON) and autosaves, even while the
    /// editor is read-only for the user.
    pub fn replace_content(&mut self, body: &str, cx: &mut Context<Self>) {
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let read_only = std::mem::replace(&mut model.read_only, false);
        // The whole document, not its first to last caret: each streamed revision replaces it.
        model.select(Selection {
            anchor: 0,
            head: model.document.units(),
        });
        let result = Document::parse(body.to_owned().into())
            .and_then(|fragment| model.insert_slice(fragment, false));
        model.read_only = read_only;
        self.edited(before, result, cx);
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        self.read_only = read_only;
        if let Some(model) = &mut self.model {
            model.read_only = read_only;
        }
        cx.notify();
    }

    pub fn flush(&mut self) -> Flush {
        if let (Some(model), Some(journal)) = (&self.model, &self.journal)
            && !model.composing()
        {
            return journal.flush(model.revision);
        }
        let (sender, receiver) = oneshot::channel();
        let _ = sender.send(Err(ServiceError::Failed(
            "Finish composition and loading before flushing the editor".into(),
        )));
        receiver
    }

    pub fn retry_save(&mut self, cx: &mut Context<Self>) {
        if let Some(journal) = &self.journal {
            journal.retry();
        }
        cx.notify();
    }

    pub(super) fn edited(&mut self, before: u64, result: EditResult<()>, cx: &mut Context<Self>) {
        Caret::reset_entity(&self.caret, cx);
        if let Err(error) = result {
            self.message = error;
        } else {
            self.message.clear();
        }
        let Some(model) = &self.model else {
            return;
        };
        if model.revision != before {
            self.dirty = true;
            if !model.composing()
                && let Some(journal) = &self.journal
            {
                journal.publish(model.revision, model.document.clone());
            }
            cx.emit(EditorEvent::Dirty {
                session_id: self.init.session_id.clone(),
                dirty: true,
            });
            let count = model.document.root.children.render_blocks();
            if count != self.list_count {
                self.list_count = count;
                self.list.reset(self.list_rows());
            } else if let Some(index) = surface::block_index(&model.document, model.selection.head)
            {
                self.list.splice(index..index + 1, 1);
            }
            self.reveal_caret();
        }
        cx.notify();
    }

    pub fn command(&mut self, command: BlockCommand, cx: &mut Context<Self>) {
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let result = Self::apply_command(model, command);
        self.edited(before, result, cx);
    }

    fn apply_command(model: &mut EditorModel, command: BlockCommand) -> EditResult<()> {
        match command {
            BlockCommand::Paragraph => model.set_block("paragraph", None),
            BlockCommand::Heading(level) => model.set_block("heading", Some(level)),
            BlockCommand::BulletList => model.wrap_block("bulletList"),
            BlockCommand::OrderedList => model.wrap_block("orderedList"),
            BlockCommand::TaskList => model.wrap_block("taskList"),
            BlockCommand::Table => model.insert_table(),
            BlockCommand::Quote => model.wrap_block("blockquote"),
            BlockCommand::Code => model.set_block("codeBlock", None),
            BlockCommand::Divider => {
                model.insert_block_atom("horizontalRule", serde_json::Map::new())
            }
        }
    }

    pub(super) fn slash_commit(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = self.slash.take() else {
            return;
        };
        let Some((command, _)) = menu.items().get(menu.selected).copied() else {
            return;
        };
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let result = model.transaction(|model| {
            model.replace(menu.start..model.selection.head, "")?;
            Self::apply_command(model, command)
        });
        self.edited(before, result, cx);
    }

    /// Escape dismisses the open "/" until it is gone, as in shipping.
    pub(super) fn update_slash(&mut self) {
        self.slash = self.find_slash();
        if self.slash.is_none() {
            self.slash_dismissed = None;
        } else if self.slash.as_ref().map(|menu| menu.start) == self.slash_dismissed {
            self.slash = None;
        }
    }

    fn find_slash(&self) -> Option<SlashMenu> {
        let model = self.model.as_ref()?;
        if model.composing() || !model.selection.is_empty() {
            return None;
        }
        let resolved = model.document.resolve(model.selection.head).ok()?;
        if resolved.node.kind() == "codeBlock" {
            return None;
        }
        let text = clipboard::text_for_range(&model.document, resolved.start..model.selection.head)
            .ok()?;
        let (slash, query) = super::menu::slash_query(&text)?;
        let start = resolved.start + text[..slash].encode_utf16().count();
        let selected = self
            .slash
            .as_ref()
            .filter(|menu| menu.query == query && menu.start == start)
            .map_or(0, |menu| menu.selected);
        Some(SlashMenu {
            start,
            query: query.into(),
            selected,
        })
    }

    pub fn enable_mentions(&mut self, enabled: bool) {
        self.mention_enabled = enabled;
    }

    pub fn use_external_mention_provider(&mut self, enabled: bool) {
        self.external_mention_provider = enabled;
    }

    pub(super) fn update_mention(&mut self, cx: &mut Context<Self>) {
        if !self.mention_enabled {
            return;
        }
        let Some(model) = &self.model else {
            return;
        };
        let query = (|| {
            if !model.selection.is_empty() || model.composing() {
                return None;
            }
            let resolved = model.document.resolve(model.selection.head).ok()?;
            if resolved.node.kind() == "codeBlock" {
                return None;
            }
            let start = resolved.start.max(model.selection.head.saturating_sub(256));
            let text =
                clipboard::text_for_range(&model.document, start..model.selection.head).ok()?;
            let (prefix, query) = text.rsplit_once('@')?;
            if prefix.chars().last().is_some_and(|c| !c.is_whitespace())
                || query.chars().any(char::is_whitespace)
            {
                return None;
            }
            Some((start + prefix.encode_utf16().count(), query.to_owned()))
        })();
        let Some((start, query)) = query else {
            self.dismiss_mention();
            return;
        };
        if self
            .mention
            .as_ref()
            .is_some_and(|menu| menu.query == query && menu.start == start)
        {
            return;
        }
        self.dismiss_mention();
        let generation = self.mention_results.begin();
        let results = std::mem::take(&mut self.mention_results);
        self.mention = Some(MentionMenu {
            start,
            query: query.clone(),
            selected: 0,
            results,
        });
        if self.external_mention_provider {
            cx.emit(EditorRequest::MentionSearch { query, generation });
        } else {
            let cancel = CancellationToken::new();
            self.mention_cancel = Some(cancel.clone());
            let job = cx.background_executor().spawn(super::services::mentions(
                self.context.runtime.clone(),
                query,
                cancel,
            ));
            cx.spawn(async move |entity, cx| {
                let result = job.await;
                let _ = entity.update(cx, |this, cx| this.resolve_mentions(generation, result, cx));
            })
            .detach();
        }
    }

    pub(super) fn dismiss_mention(&mut self) {
        if let Some(cancel) = self.mention_cancel.take() {
            cancel.cancel();
        }
        if let Some(mut menu) = self.mention.take() {
            menu.results.dismiss();
            self.mention_results = menu.results;
        }
    }

    pub fn resolve_mentions(
        &mut self,
        generation: u64,
        result: Result<Vec<MentionCandidate>, String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(menu) = &mut self.mention
            && menu.results.resolve(generation, result)
        {
            cx.notify();
        }
    }

    pub(super) fn commit_mention(&mut self, cx: &mut Context<Self>) {
        let Some(menu) = &self.mention else {
            return;
        };
        let Some(candidate) = menu.results.candidates.get(menu.selected).cloned() else {
            return;
        };
        let start = menu.start;
        self.dismiss_mention();
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let (kind, id) = match candidate.target {
            MentionTarget::Human(id) => ("human", id),
            MentionTarget::Session(id) => ("session", id),
            MentionTarget::Organization(id) => ("organization", id),
        };
        model.select(Selection {
            anchor: start,
            head: model.selection.head,
        });
        let result = model.insert_inline_atom(
            "mention-@",
            serde_json::Map::from_iter([
                ("id".into(), serde_json::json!(id)),
                ("type".into(), serde_json::json!(kind)),
                ("label".into(), serde_json::json!(candidate.label)),
            ]),
        );
        self.edited(before, result, cx);
    }

    pub fn insert_attachment(
        &mut self,
        id: AttachmentId,
        name: String,
        image: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(model) = &mut self.model else {
            return;
        };
        let before = model.revision;
        let attrs = serde_json::Map::from_iter([
            ("attachmentId".into(), serde_json::json!(id.0.as_ref())),
            (
                if image { "alt" } else { "name" }.into(),
                serde_json::json!(name),
            ),
        ]);
        let result = model.insert_block_atom(if image { "image" } else { "fileAttachment" }, attrs);
        self.edited(before, result, cx);
    }

    pub fn set_footer(&mut self, footer: Option<gpui::AnyView>, cx: &mut Context<Self>) {
        self.footer = footer;
        self.list.reset(self.list_rows());
        cx.notify();
    }

    fn list_rows(&self) -> usize {
        self.list_count + usize::from(self.footer.is_some())
    }

    pub fn configure_attachments(&mut self, vault: std::path::PathBuf, cx: &mut Context<Self>) {
        self.attachment_service = Some(super::AttachmentService::new(
            self.context.runtime.clone(),
            vault,
        ));
        self.attachment_previews.clear();
        cx.notify();
    }

    pub fn open_attachment(&mut self, id: AttachmentId, cx: &mut Context<Self>) {
        let Some(service) = self.attachment_service.clone() else {
            cx.emit(EditorEvent::OpenAttachment(id));
            return;
        };
        let session = self.init.session_id.clone();
        let job = cx.background_executor().spawn(async move {
            let preview = service
                .resolve(session, id.0.to_string(), CancellationToken::new())
                .await?;
            crate::platform::os::open(preview.path.as_ref()).map_err(|error| error.to_string())
        });
        cx.spawn(async move |entity, cx| {
            if let Err(error) = job.await {
                let _ = entity.update(cx, |this, cx| {
                    this.message = error;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn choose_attachment(&mut self, cx: &mut Context<Self>) {
        if self.attachment_importing || self.read_only {
            return;
        }
        let Some(service) = self.attachment_service.clone() else {
            return;
        };
        self.attachment_importing = true;
        let session = self.init.session_id.clone();
        let job = cx.background_executor().spawn(async move {
            let Some(file) =
                crate::platform::os::dialog(|| rfd::AsyncFileDialog::new().pick_file()).await
            else {
                return Ok(None);
            };
            service
                .import(session, file.path().to_owned())
                .await
                .map(Some)
        });
        cx.spawn(async move |entity, cx| {
            let result = job.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(Some(preview)) => {
                        this.cache_attachment_preview(preview.id.clone(), Ok(preview.clone()));
                        this.pending_attachment = Some(preview);
                        this.insert_pending_attachment(cx);
                    }
                    Ok(None) => {}
                    Err(error) => this.message = error,
                }
                this.attachment_importing = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn paste_image(&mut self, image: gpui::Image, cx: &mut Context<Self>) {
        if self.attachment_importing {
            self.message =
                "Wait for the current attachment import before pasting another image.".into();
            return;
        }
        let Some(service) = self.attachment_service.clone() else {
            self.message = "Configure the active vault before importing attachments.".into();
            return;
        };
        self.attachment_importing = true;
        let session = self.init.session_id.clone();
        let job = cx.background_executor().spawn(async move {
            let extension = image
                .format
                .mime_type()
                .split('/')
                .nth(1)
                .unwrap_or("png")
                .replace("+xml", "");
            service
                .import_bytes(session, format!("pasted-image.{extension}"), image.bytes)
                .await
        });
        cx.spawn(async move |entity, cx| {
            let result = job.await;
            let _ = entity.update(cx, |this, cx| {
                match result {
                    Ok(preview) => {
                        this.cache_attachment_preview(preview.id.clone(), Ok(preview.clone()));
                        this.pending_attachment = Some(preview);
                        this.insert_pending_attachment(cx);
                    }
                    Err(error) => this.message = error,
                }
                this.attachment_importing = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn cache_attachment_preview(
        &mut self,
        id: String,
        preview: Result<super::AttachmentPreview, String>,
    ) {
        if self.attachment_previews.len() >= 128 {
            self.attachment_previews.clear();
        }
        self.attachment_previews.insert(id, preview);
    }

    fn insert_pending_attachment(&mut self, cx: &mut Context<Self>) {
        let (Some(preview), Some(model)) = (&self.pending_attachment, &mut self.model) else {
            return;
        };
        let (kind, attrs) = attachment_node(preview);
        let before = model.revision;
        let result = model.insert_block_atom(kind, attrs);
        if result.is_ok() {
            self.pending_attachment = None;
        }
        self.edited(before, result, cx);
    }

    fn open_attachment_file(&mut self, path: Arc<std::path::PathBuf>, cx: &mut Context<Self>) {
        let job = cx.background_executor().spawn(async move {
            crate::platform::os::open(path.as_ref()).map_err(|error| error.to_string())
        });
        cx.spawn(async move |entity, cx| {
            if let Err(error) = job.await {
                let _ = entity.update(cx, |this, cx| {
                    this.message = error;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn attachment_remove_control(
        &self,
        start: usize,
        id: u64,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id(("remove-attachment", id))
            .text_sm()
            .cursor_pointer()
            .when(!self.read_only, |view| {
                view.child("Remove")
                    .a11y_button("Remove attachment")
                    .a11y_press(cx, move |this, _, cx| {
                        if let Some(model) = &mut this.model {
                            let before = model.revision;
                            let result = model.remove_block_atom(start, id);
                            this.edited(before, result, cx);
                        }
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if let Some(model) = &mut this.model {
                                let before = model.revision;
                                let result = model.remove_block_atom(start, id);
                                this.edited(before, result, cx);
                            }
                            this.dragging = false;
                            cx.stop_propagation();
                        }),
                    )
            })
            .into_any_element()
    }

    /// Shipping's `ResizableImageView`: `editorWidth` percent of the column (80 by default),
    /// a ring on hover or selection, and side handles that resize between 120px and the
    /// column's width.
    fn render_image(
        &mut self,
        node: &super::document::NodeRef,
        start: usize,
        source: gpui::ImageSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let colors = theme(window);
        let id = node.id;
        let hovered = self.hovered_image == Some(id);
        let selected = self
            .selected_image
            .is_some_and(|(_, selected)| selected == id);
        let draft = self
            .resize
            .as_ref()
            .filter(|resize| resize.id == id)
            .map(|resize| resize.width);
        let controls = !self.read_only && (hovered || selected || draft.is_some());
        let percent = image_width(node);
        let entity = cx.entity();
        let handle = |left: bool, cx: &mut Context<Self>| {
            div()
                .id(("image-resize", id * 2 + u64::from(left)))
                .absolute()
                .top(gpui::relative(0.5))
                .mt(px(-28.))
                .when(left, |view| view.left(px(4.)))
                .when(!left, |view| view.right(px(4.)))
                .h(px(56.))
                .w(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(colors.border)
                .bg(colors.card.opacity(0.95))
                .shadow_sm()
                .cursor(gpui::CursorStyle::ResizeLeftRight)
                .child(div().h(px(32.)).w(px(4.)).bg(colors.muted_foreground))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &gpui::MouseDownEvent, _, cx| {
                        let initial = this.image_widths.get(&id).copied().unwrap_or(px(120.));
                        let max = this
                            .viewport
                            .map_or(initial, |viewport| viewport.size.width - px(24.))
                            .max(px(120.));
                        this.resize = Some(ImageResize {
                            id,
                            start,
                            left,
                            origin: event.position.x,
                            initial,
                            max,
                            width: initial,
                        });
                        this.dragging = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
        };
        div()
            .w_full()
            .px(px(12.))
            .py_1()
            .child(
                div()
                    .id(("image", id))
                    .relative()
                    .max_w_full()
                    .map(|view| match draft {
                        Some(width) => view.w(width),
                        None => view.w(gpui::relative((percent / 100.) as f32)),
                    })
                    .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                        let next = if *hovered {
                            Some(id)
                        } else {
                            this.hovered_image.filter(|current| *current != id)
                        };
                        if this.hovered_image != next {
                            this.hovered_image = next;
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.selected_image = Some((start, id));
                            this.focus.focus(window, cx);
                            this.dragging = false;
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(
                        gpui::img(source)
                            .w_full()
                            .bg(colors.card)
                            .object_fit(gpui::ObjectFit::Contain),
                    )
                    // `ring-1 ring-offset-2`: a 1px ring 2px outside the image.
                    .when(hovered || selected, |view| {
                        view.child(
                            div()
                                .absolute()
                                .top(px(-3.))
                                .left(px(-3.))
                                .right(px(-3.))
                                .bottom(px(-3.))
                                .border_1()
                                .border_color(if selected {
                                    colors.foreground.opacity(0.55)
                                } else {
                                    colors.border
                                }),
                        )
                    })
                    .when(controls, |view| {
                        view.child(handle(true, cx)).child(handle(false, cx))
                    })
                    .child(
                        canvas(
                            move |bounds, _, cx| {
                                entity.update(cx, |this, _| {
                                    this.image_widths.insert(id, bounds.size.width);
                                })
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
            .into_any_element()
    }

    fn render_attachment(
        &mut self,
        node: &super::document::NodeRef,
        start: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !matches!(node.kind(), "image" | "fileAttachment") {
            return None;
        }
        if node.kind() == "image"
            && node
                .attr("attachmentId")
                .and_then(serde_json::Value::as_str)
                .is_none()
            && let Some(src) = node
                .attr("src")
                .and_then(serde_json::Value::as_str)
                .filter(|src| clipboard::openable_link(src))
        {
            let source = gpui::ImageSource::from(src.to_owned());
            return Some(self.render_image(node, start, source, window, cx));
        }
        let id = node
            .attr("attachmentId")
            .and_then(serde_json::Value::as_str)
            .filter(|id| !id.is_empty())
            .or_else(|| {
                node.attr("sharedAttachmentId")
                    .and_then(serde_json::Value::as_str)
            })?
            .to_owned();
        if !self.attachment_previews.contains_key(&id)
            && self.attachment_loading.len() < 4
            && !self.attachment_loading.contains(&id)
            && let Some(service) = self.attachment_service.clone()
        {
            self.attachment_loading.insert(id.clone());
            let session = self.init.session_id.clone();
            let request_id = id.clone();
            let cancel = self.watch_cancel.clone();
            let job = cx
                .background_executor()
                .spawn(async move { service.resolve(session, request_id, cancel).await });
            let request_id = id.clone();
            cx.spawn(async move |entity, cx| {
                let preview = job.await;
                let _ = entity.update(cx, |this, cx| {
                    this.attachment_loading.remove(&request_id);
                    this.cache_attachment_preview(request_id, preview);
                    cx.notify();
                });
            })
            .detach();
        }
        match self.attachment_previews.get(&id) {
            Some(Ok(preview)) if node.kind() == "image" && preview.mime.starts_with("image/") => {
                let source = gpui::ImageSource::from(preview.path.as_ref().clone());
                Some(self.render_image(node, start, source, window, cx))
            }
            Some(Ok(preview)) => {
                let path = preview.path.clone();
                let image = preview.mime.starts_with("image/");
                Some(
                    div()
                        .id(("attachment", node.id))
                        .my_1()
                        .border_1()
                        .px_3()
                        .py(px(10.))
                        .flex()
                        .items_center()
                        .gap_3()
                        .cursor_pointer()
                        .a11y_button(format!(
                            "{}, {:.1} KB",
                            preview.name,
                            preview.size as f64 / 1024.
                        ))
                        .a11y_press(cx, {
                            let path = path.clone();
                            move |this, _, cx| this.open_attachment_file(path.clone(), cx)
                        })
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.open_attachment_file(path.clone(), cx);
                                this.dragging = false;
                                cx.stop_propagation();
                            }),
                        )
                        .when(image, |view| {
                            view.child(
                                gpui::img(preview.path.as_ref().clone())
                                    .w(px(40.))
                                    .h(px(40.))
                                    .object_fit(gpui::ObjectFit::Contain),
                            )
                        })
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_sm()
                                .child(preview.name.clone())
                                .child(
                                    div()
                                        .text_xs()
                                        .child(format!("{:.1} KB", preview.size as f64 / 1024.)),
                                ),
                        )
                        .child(self.attachment_remove_control(start, node.id, cx))
                        .into_any_element(),
                )
            }
            Some(Err(error)) => Some(
                div()
                    .p_3()
                    .border_1()
                    .child(error.clone())
                    .child(self.attachment_remove_control(start, node.id, cx))
                    .into_any_element(),
            ),
            None => None,
        }
    }

    pub(super) fn copy(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(model) = &self.model else {
            return;
        };
        let document = model.document.clone();
        let selection = model.selection;
        let revision = model.revision;
        let source = self.init.document.id.0.clone();
        self.clipboard_generation += 1;
        let generation = self.clipboard_generation;
        let job = cx
            .background_executor()
            .spawn(async move { clipboard::copy(&document, selection, &source) });
        cx.spawn(async move |entity, cx| {
            let result = job.await;
            let _ = entity.update(cx, |this, cx| {
                if generation != this.clipboard_generation {
                    return;
                }
                match result {
                    Ok(payload) => {
                        cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(
                            payload.text,
                            payload.metadata,
                        ));
                        if cut && let Some(model) = &mut this.model {
                            if model.revision != revision || model.selection != selection {
                                this.message =
                                    "Selection changed while copying; copied content was not cut."
                                        .into();
                            } else {
                                let result = model.replace(selection.range(), "");
                                this.edited(revision, result, cx);
                            }
                        }
                    }
                    Err(error) => this.message = error,
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(super) fn paste(
        &mut self,
        text: String,
        metadata: Option<String>,
        rich: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(model) = &mut self.model else {
            return;
        };
        let revision = model.revision;
        let selection = model.selection;
        self.clipboard_generation += 1;
        let generation = self.clipboard_generation;
        // Like ProseMirror, code takes the clipboard's plain text, whatever rich content came
        // with it.
        if model
            .document
            .resolve(selection.head)
            .is_ok_and(|resolved| resolved.node.kind() == "codeBlock")
        {
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            let result = model.replace(selection.range(), &text);
            self.edited(revision, result, cx);
            return;
        }
        let destination = self.init.document.id.0.clone();
        let job = cx.background_executor().spawn(async move {
            if let Some(metadata) = metadata { return clipboard::parse_slice(&metadata, &destination); }
            if rich
                && let Ok(mut clipboard) = arboard::Clipboard::new()
                && clipboard.get_text().is_ok_and(|current| current == text)
                && let Ok(html) = clipboard.get().html()
            {
                return super::html::parse(&html);
            }
            if text.len() > 1024 * 1024 { return Err("Paste exceeds the 1 MiB transaction limit".into()); }
            let content: Vec<_> = text.split('\n').map(|line| serde_json::json!({
                "type": "paragraph", "content": if line.is_empty() { Vec::new() }
                else { vec![serde_json::json!({"type": "text", "text": line.trim_end_matches('\r')})] }
            })).collect();
            Document::parse(serde_json::json!({"type":"doc","content":content}).to_string().into()).map(|document| (document, true))
        });
        cx.spawn(async move |entity, cx| {
            let fragment = job.await;
            let _ = entity.update(cx, |this, cx| {
                if this.clipboard_generation != generation { return; }
                let Some(model) = &mut this.model else { return; };
                if model.revision != revision || model.selection != selection {
                    this.message = "Selection changed while preparing paste. Paste again at the desired position.".into();
                    cx.notify(); return;
                }
                let result = fragment.and_then(|(fragment, open)| model.insert_slice(fragment, open));
                this.edited(revision, result, cx);
            });
        }).detach();
    }

    fn fetch_conflict(&mut self, cx: &mut Context<Self>) {
        self.external_generation += 1;
        let generation = self.external_generation;
        let reply = self
            .context
            .runtime
            .open_session(self.init.session_id.clone(), CancellationToken::new());
        cx.spawn(async move |entity, cx| {
            let result = match reply {
                Ok(reply) => reply.receive().await,
                Err(error) => Err(error),
            };
            let _ = entity.update(cx, |this, cx| {
                if this.external_generation != generation {
                    return;
                }
                match result {
                    Ok(session) => this.external = session.note,
                    Err(error) => {
                        this.message =
                            format!("Draft retained. Cannot load the conflicting revision: {error}")
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub fn merge_external(&mut self, cx: &mut Context<Self>) {
        let (Some(model), Some(remote)) = (&self.model, self.external.clone()) else {
            return;
        };
        if model.composing() || model.read_only {
            self.message = "Finish composition and enable editing before merging".into();
            cx.notify();
            return;
        }
        let revision = model.revision;
        let selection = model.selection;
        let generation = self.external_generation;
        let local = model.document.clone();
        let base = self.init.document.body.clone();
        let remote_body = remote.body.clone();
        let job = cx.background_executor().spawn(async move {
            let base: serde_json::Value = serde_json::from_str(&base).map_err(|error| error.to_string())?;
            let remote: serde_json::Value = serde_json::from_str(&remote_body).map_err(|error| error.to_string())?;
            let local = local.root.value();
            for value in [&base, &remote, &local] {
                if value["content"].as_array().is_none_or(|blocks| blocks.len() > 2000) {
                    return Err("Automatic merge is limited to 2000 blocks. Draft and remote revision retained.".into());
                }
            }
            let outcome = anlg_tiptap::merge::merge_documents(&base, &local, &remote, anlg_tiptap::merge::MergeSide::Local).ok_or("Cannot merge this document shape")?;
            if !outcome.conflicts.is_empty() {
                return Err(format!("{} overlapping block conflicts require manual resolution. Both revisions retained.", outcome.conflicts.len()));
            }
            Document::parse(outcome.doc.to_string().into())
        });
        cx.spawn(async move |entity, cx| {
            let result = job.await;
            let _ = entity.update(cx, |this, cx| {
                if this.external_generation != generation { return; }
                if this.model.as_ref().is_none_or(|model| model.revision != revision || model.composing()) {
                    this.message = "Draft changed during merge. Retry merge when typing has stopped.".into(); cx.notify(); return;
                }
                match result {
                    Ok(document) => {
                        let next = revision + 1;
                        let result = this.journal.as_ref().ok_or(ServiceError::Closed).and_then(|journal| journal.resolve_conflict(remote.clone(), next, document.clone()));
                        match result {
                            Ok(()) => {
                                let mut model = EditorModel::new(document);
                                model.revision = next;
                                model.read_only = this.read_only;
                                if model.document.resolve(selection.head).is_ok() && model.document.resolve(selection.anchor).is_ok() { model.select(selection); }
                                this.list_count = model.document.root.children.render_blocks();
                                this.list.reset(this.list_rows());
                                this.model = Some(model);
                                this.init.document = remote;
                                this.external = None;
                                this.projection.clear();
                                this.message = "Nonoverlapping changes merged; saving. Undo history reset after merge.".into();
                            }
                            Err(error) => this.message = error.to_string(),
                        }
                    }
                    Err(error) => this.message = error,
                }
                cx.notify();
            });
        }).detach();
    }

    pub fn apply_external_revision(
        &mut self,
        snapshot: DocumentSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focused = self.focus.is_focused(window);
        self.remote_revision(snapshot, cx);
    }

    pub fn remote_revision(&mut self, snapshot: DocumentSnapshot, cx: &mut Context<Self>) {
        if snapshot.id != self.init.document.id
            || snapshot.session_id != self.init.session_id
            || snapshot.updated_at == self.init.document.updated_at
        {
            return;
        }
        self.external_generation += 1;
        if self.dirty || self.focused || self.model.as_ref().is_some_and(EditorModel::composing) {
            self.external = Some(snapshot);
            if self.save_failed {
                // A failed save plus another revision is the note banner's save conflict.
                self.message.clear();
            } else {
                self.message = "A remote revision is available. Your draft and composition are retained; resolve before saving.".into();
            }
            cx.notify();
            return;
        }
        self.load_clean_revision(snapshot, cx);
    }

    fn load_clean_revision(&mut self, snapshot: DocumentSnapshot, cx: &mut Context<Self>) {
        if snapshot.body_format.as_ref() != "prosemirror_json" {
            self.message = "Unsupported remote format retained; local document unchanged".into();
            self.external = Some(snapshot);
            cx.notify();
            return;
        }
        let generation = self.external_generation;
        let body = with_title(snapshot.body.clone(), self.title.as_deref());
        let job = cx
            .background_executor()
            .spawn(async move { Document::parse(body) });
        cx.spawn(async move |entity, cx| {
            let parsed = job.await;
            let _ = entity.update(cx, |this, cx| {
                if this.external_generation != generation { return; }
                if this.dirty || this.focused || this.model.as_ref().is_some_and(EditorModel::composing) {
                    this.external = Some(snapshot); return;
                }
                match parsed {
                    Ok(document) if this.journal.as_ref().is_some_and(|journal| journal.rebase_if_clean(snapshot.clone())) => {
                        this.list_count = document.root.children.render_blocks();
                        this.list.reset(this.list_rows());
                        let mut model = EditorModel::new(document);
                        model.read_only = this.read_only;
                        this.model = Some(model);
                        this.init.document = snapshot;
                        this.projection.clear();
                        this.message = "Remote revision loaded. Undo history was reset to protect remote edits.".into();
                        this.external = None;
                    }
                    Ok(_) => { this.external = Some(snapshot); }
                    Err(error) => { this.message = error; this.external = Some(snapshot); }
                }
                cx.notify();
            });
        }).detach();
    }

    fn render_block(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        if index >= self.list_count
            && let Some(footer) = &self.footer
        {
            return footer.clone().into_any_element();
        }
        let Some(model) = &self.model else {
            return div().into_any_element();
        };
        let Some((node, start, style)) = surface::block_layout(&model.document, index) else {
            return div().into_any_element();
        };
        if let Some(preview) = self.render_attachment(&node, start, window, cx) {
            return preview;
        }
        let cached = self.projection.get(&node.id).cloned();
        let current = cached
            .as_ref()
            .is_some_and(|cached| Arc::ptr_eq(&cached.source, &node) && cached.style == style);
        if !current && self.projecting.len() < 8 && self.projecting.insert(node.id) {
            let id = node.id;
            let job = cx
                .background_executor()
                .spawn(async move { Arc::new(ProjectedBlock::with_layout(node, style)) });
            cx.spawn(async move |entity, cx| {
                let projected = job.await;
                let _ = entity.update(cx, |this, cx| {
                    this.projecting.remove(&id);
                    if this.projection.len() >= 128 {
                        let visible: HashSet<_> = this.layouts.keys().copied().collect();
                        this.projection.retain(|_, block| {
                            block.rows.iter().any(|row| visible.contains(&row.id))
                        });
                        if this.projection.len() >= 128 {
                            this.projection.clear();
                        }
                    }
                    this.projection.insert(id, projected);
                    if index < this.list_count {
                        this.list.splice(index..index + 1, 1);
                    }
                    if this.pending_reveal.is_some() {
                        this.reveal_caret();
                    }
                    cx.notify();
                });
            })
            .detach();
        }
        // Shipping's placeholder plugin labels the empty text block holding the selection anchor.
        let anchor = self.model.as_ref().map(|model| model.selection.anchor);
        let placeholder_text = self.placeholder.clone();
        let first_row = cached
            .as_ref()
            .and_then(|block| block.rows.first())
            .map(|row| row.id);
        let titled = self.title.is_some() && index == 0;
        let is_title = |row: &surface::Row| {
            titled && Some(row.id) == first_row && row.kind == "heading" && row.level == 1
        };
        let placeholder = |row: &surface::Row| {
            (row.text.is_empty() && anchor == Some(start + row.start))
                .then(|| placeholder_text.clone())
                .flatten()
        };
        if let Some(block) = &cached {
            super::app_link::load_states(&block.rows, &self.context.runtime, cx);
        }
        match cached {
            Some(block) if block.grid.is_some() => {
                let grid = block.grid.as_ref().expect("grid");
                let colors = theme(window);
                div()
                    .grid()
                    .grid_cols(grid.columns)
                    .w_full()
                    .my_4()
                    .border_t_1()
                    .border_l_1()
                    .border_color(colors.border)
                    .children(grid.cells.iter().map(|cell| {
                        div()
                            .col_span(cell.colspan)
                            .col_start(cell.column)
                            .row_span(cell.rowspan)
                            .row_start(cell.row)
                            .min_w_0()
                            .border_r_1()
                            .border_b_1()
                            .border_color(colors.border)
                            .py_2()
                            .px_3()
                            .when(cell.header, |view| {
                                view.bg(colors.muted)
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                            })
                            .children(block.rows[cell.rows.clone()].iter().map(|row| {
                                if let Some(preview) = row.spans.first().and_then(|span| {
                                    self.render_attachment(
                                        &span.node,
                                        start + row.start + span.position,
                                        window,
                                        cx,
                                    )
                                }) {
                                    preview
                                } else {
                                    surface::render_row(
                                        row.clone(),
                                        start,
                                        current,
                                        surface::RowPlace::Cell,
                                        placeholder(row),
                                        window,
                                        cx,
                                    )
                                }
                            }))
                    }))
                    .into_any_element()
            }
            Some(block) => div()
                .w_full()
                .children(block.rows.iter().map(|row| {
                    if let Some(preview) = row.spans.first().and_then(|span| {
                        self.render_attachment(
                            &span.node,
                            start + row.start + span.position,
                            window,
                            cx,
                        )
                    }) {
                        preview
                    } else {
                        surface::render_row(
                            row.clone(),
                            start,
                            current,
                            if is_title(row) {
                                surface::RowPlace::Title
                            } else {
                                surface::RowPlace::Body
                            },
                            placeholder(row).or_else(|| {
                                (is_title(row) && row.text.is_empty()).then(|| "Untitled".into())
                            }),
                            window,
                            cx,
                        )
                    }
                }))
                .into_any_element(),
            // Layout runs off the UI thread for a frame; shipping shows no placeholder text.
            None => div().h(px(28.)).into_any_element(),
        }
    }

    pub(super) fn hit(&self, position: Point<Pixels>) -> Option<usize> {
        self.layouts
            .values()
            .find(|layout| layout.layout.bounds().contains(&position))
            .map(|layout| layout.position(position))
            .or_else(|| {
                self.layouts
                    .values()
                    .min_by(|a, b| {
                        let da = (f32::from(a.layout.bounds().top()) - f32::from(position.y)).abs();
                        let db = (f32::from(b.layout.bounds().top()) - f32::from(position.y)).abs();
                        da.total_cmp(&db)
                    })
                    .map(|layout| layout.position(position))
            })
    }

    pub(super) fn reveal_caret(&mut self) {
        if let Some(model) = &self.model
            && let Some(index) = surface::block_index(&model.document, model.selection.head)
        {
            let head = model.selection.head;
            self.pending_reveal = Some(head);
            if !self
                .layouts
                .values()
                .any(|layout| layout.bounds_for(head..head).is_some())
            {
                self.list.scroll_to(ListOffset {
                    item_ix: index,
                    offset_in_item: px(0.),
                });
            }
        }
    }

    fn finish_reveal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(head) = self.pending_reveal else {
            return;
        };
        let Some(viewport) = self.viewport else {
            return;
        };
        let Some(caret) = self
            .layouts
            .values()
            .find_map(|layout| layout.bounds_for(head..head))
        else {
            return;
        };
        let distance = if caret.top() < viewport.top() {
            caret.top() - viewport.top()
        } else if caret.bottom() > viewport.bottom() {
            caret.bottom() - viewport.bottom()
        } else {
            px(0.)
        };
        if distance == px(0.) {
            if self.projecting.is_empty() {
                self.pending_reveal = None;
            }
        } else {
            let entity = cx.weak_entity();
            window.on_next_frame(move |_, cx| {
                let _ = entity.update(cx, |this, cx| {
                    if this
                        .model
                        .as_ref()
                        .is_some_and(|model| model.selection.head == head)
                    {
                        this.list.scroll_by(distance);
                        cx.notify();
                    }
                });
            });
        }
    }

    pub(super) fn commit_link(&mut self, cx: &mut Context<Self>) {
        if let Some(href) = self.link_input.clone()
            && let Some(model) = &mut self.model
        {
            let before = model.revision;
            let result = model.set_link(&href);
            if result.is_ok() {
                self.link_input = None;
            }
            self.edited(before, result, cx);
        }
    }

    fn start_drag_scroll(&mut self, cx: &mut Context<Self>) {
        if self.drag_scroll.is_some() {
            return;
        }
        self.drag_scroll = Some(cx.spawn(async move |entity, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(30))
                    .await;
                let keep_running = entity
                    .update(cx, |this, cx| {
                        if !this.dragging {
                            return false;
                        }
                        if let (Some(point), Some(bounds)) = (this.drag_position, this.viewport) {
                            let delta = if point.y < bounds.top() + px(32.) {
                                16.
                            } else if point.y > bounds.bottom() - px(32.) {
                                -16.
                            } else {
                                0.
                            };
                            if delta != 0. {
                                this.list.scroll_by(px(delta));
                                if let Some(position) = this.hit(point)
                                    && let Some(model) = &mut this.model
                                {
                                    model.select(Selection {
                                        anchor: model.selection.anchor,
                                        head: position,
                                    });
                                }
                                cx.notify();
                            }
                        }
                        true
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
            }
        }));
    }

    fn activate_at(&mut self, position: usize, cx: &mut Context<Self>) {
        for layout in self.layouts.values() {
            for span in &layout.row.spans {
                let start = layout.block_start + layout.row.start + span.position;
                if !(start..start + span.units).contains(&position) {
                    continue;
                }
                if let Some(id) = span
                    .node
                    .attr("attachmentId")
                    .and_then(serde_json::Value::as_str)
                {
                    self.open_attachment(AttachmentId(id.into()), cx);
                    return;
                }
                if span.node.kind() == "appLink" {
                    let resources = self.model.as_ref().map(|model| {
                        crate::workspace::task::sibling_resources(
                            &model.document,
                            start,
                            &span.node.value(),
                        )
                    });
                    match resources.filter(|resources| !resources.is_empty()) {
                        Some(resources) => cx.emit(EditorRequest::OpenTask(
                            crate::workspace::task::route(&resources),
                        )),
                        None => {
                            if let Some(url) =
                                span.node.attr("url").and_then(serde_json::Value::as_str)
                                && clipboard::openable_link(url)
                            {
                                cx.emit(EditorEvent::OpenLink(url.into()));
                            }
                        }
                    }
                    return;
                }
                if span.node.kind() == "mention-@"
                    && span.node.attr("type").and_then(serde_json::Value::as_str) == Some("human")
                {
                    if let Some(id) = span.node.attr("id").and_then(serde_json::Value::as_str) {
                        cx.emit(EditorEvent::MentionHuman(HumanId(id.into())));
                    }
                    return;
                }
                if span.node.kind() == "mention-@"
                    && let Some(id) = span.node.attr("id").and_then(serde_json::Value::as_str)
                {
                    match span.node.attr("type").and_then(serde_json::Value::as_str) {
                        Some("session") => cx.emit(EditorRequest::OpenSession(id.into())),
                        Some("organization") => cx.emit(EditorRequest::OpenOrganization(id.into())),
                        _ => {}
                    }
                    return;
                }
                for mark in &span.marks {
                    if mark["type"] == "link"
                        && let Some(href) = mark["attrs"]["href"].as_str()
                        && clipboard::openable_link(href)
                    {
                        cx.emit(EditorEvent::OpenLink(href.into()));
                        return;
                    }
                }
            }
        }
    }
}

impl EventEmitter<EditorEvent> for EditorPane {}
impl EventEmitter<EditorRequest> for EditorPane {}
impl Focusable for EditorPane {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for EditorPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_find();
        if !cx.has_active_drag() {
            self.drop_cursor = None;
            self.drop_declined = false;
        }
        self.caret.update(cx, |caret, cx| {
            caret.set_enabled(
                !self.read_only
                    && self
                        .model
                        .as_ref()
                        .is_some_and(|model| model.selection.is_empty())
                    && self.link_input.is_none(),
                window,
                cx,
            );
        });
        let caret = self.model.as_ref().and_then(|model| {
            self.layouts
                .values()
                .find_map(|layout| layout.bounds_for(model.selection.head..model.selection.head))
        });
        let anchor_at = |position: usize| {
            self.layouts
                .values()
                .find_map(|layout| layout.bounds_for(position..position))
        };
        let popup_position = |anchor: Option<gpui::Bounds<Pixels>>,
                              width: f32,
                              height: f32,
                              below: bool,
                              gap: f32| {
            if let (Some(anchor), Some(viewport)) = (anchor, self.viewport) {
                let left = (anchor.left() - viewport.left())
                    .max(px(8.))
                    .min((viewport.size.width - px(width + 8.)).max(px(8.)));
                let under = anchor.bottom() - viewport.top() + px(gap);
                let above = anchor.top() - viewport.top() - px(height + gap);
                let top = if below && under + px(height) < viewport.size.height {
                    under
                } else if above >= px(8.) {
                    above
                } else {
                    under
                };
                gpui::point(left, top)
            } else {
                gpui::point(px(12.), px(40.))
            }
        };
        let trigger = self
            .slash
            .as_ref()
            .map(|menu| menu.start)
            .or(self.mention.as_ref().map(|menu| menu.start));
        let menu_position =
            popup_position(trigger.and_then(anchor_at).or(caret), 280., 256., true, 4.);
        let toolbar_position = popup_position(caret, 186., 36., false, 8.);
        self.layouts.clear();
        let colors = theme(window);
        let entity = cx.entity();
        let handler = entity.clone();
        let viewport_entity = entity.clone();
        let menu =
            self.slash.as_ref().and_then(|menu| {
                let items = menu.items();
                if items.is_empty() {
                    return None;
                }
                Some(
                    div()
                        .id("slash-menu")
                        .a11y_group(gpui::Role::ListBox, "Commands")
                        .absolute()
                        .left(menu_position.x)
                        .top(menu_position.y)
                        .w(px(224.))
                        .max_h(px(256.))
                        .overflow_y_scroll()
                        .p_1()
                        .border_1()
                        .border_color(colors.border)
                        .bg(colors.popover)
                        .text_color(colors.popover_foreground)
                        .shadow_lg()
                        .when(!items.is_empty(), |view| {
                            view.child(
                                div()
                                    .px_2()
                                    .pb(px(2.))
                                    .text_size(px(10.))
                                    .line_height(px(15.))
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(colors.muted_foreground)
                                    .child("COMMANDS"),
                            )
                        })
                        .children(items.into_iter().enumerate().map(
                            |(index, (command, label))| {
                                div()
                                    .id(("slash-option", index))
                                    .a11y_option(label, index == menu.selected)
                                    .a11y_press(cx, move |this, _, cx| {
                                        if let Some(menu) = &mut this.slash {
                                            menu.selected = index;
                                        }
                                        this.slash_commit(cx);
                                    })
                                    .h(px(32.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .cursor_pointer()
                                    .when(index == menu.selected, |view| view.bg(colors.muted))
                                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                                        if let Some(menu) = &mut this.slash
                                            && menu.selected != index
                                        {
                                            menu.selected = index;
                                            cx.notify();
                                        }
                                    }))
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |this, _, _, cx| {
                                            if let Some(menu) = &mut this.slash {
                                                menu.selected = index;
                                            }
                                            this.slash_commit(cx);
                                            cx.stop_propagation();
                                        }),
                                    )
                                    .child(
                                        gpui::svg()
                                            .path(command.icon())
                                            .size(px(16.))
                                            .flex_shrink_0()
                                            .text_color(colors.muted_foreground),
                                    )
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex_1()
                                            .truncate()
                                            .text_sm()
                                            .line_height(px(16.))
                                            .child(label),
                                    )
                            },
                        )),
                )
            });
        let mentions = self
            .mention
            .as_ref()
            .filter(|menu| !menu.results.candidates.is_empty() || menu.results.error.is_some())
            .map(|menu| {
                div()
                    .id("mention-menu")
                    .a11y_group(gpui::Role::ListBox, "Mentions")
                    .absolute()
                    .left(menu_position.x)
                    .top(menu_position.y)
                    .max_w(px(280.))
                    .min_w(px(160.))
                    .p_1()
                    .border_1()
                    .border_color(colors.border)
                    .bg(colors.popover)
                    .text_color(colors.popover_foreground)
                    .shadow_lg()
                    .children(menu.results.candidates.iter().enumerate().map(
                        |(index, candidate)| {
                            let icon = match candidate.target {
                                MentionTarget::Human(_) => "editor/UserIcon.svg",
                                MentionTarget::Session(_) => "editor/NoteIcon.svg",
                                MentionTarget::Organization(_) => "editor/Building03Icon.svg",
                            };
                            div()
                                .id(("mention-option", index))
                                .a11y_option(candidate.label.clone(), index == menu.selected)
                                .a11y_press(cx, move |this, _, cx| {
                                    if let Some(menu) = &mut this.mention {
                                        menu.selected = index;
                                    }
                                    this.commit_mention(cx);
                                })
                                .h(px(32.))
                                .px_2()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .cursor_pointer()
                                .when(index == menu.selected, |view| view.bg(colors.muted))
                                .hover(|view| view.bg(colors.muted))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        if let Some(menu) = &mut this.mention {
                                            menu.selected = index;
                                        }
                                        this.commit_mention(cx);
                                        cx.stop_propagation();
                                    }),
                                )
                                .child(
                                    gpui::svg()
                                        .path(icon)
                                        .size(px(16.))
                                        .flex_shrink_0()
                                        .text_color(colors.muted_foreground),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .truncate()
                                        .text_sm()
                                        .line_height(px(16.))
                                        .child(candidate.label.clone()),
                                )
                        },
                    ))
                    .children(menu.results.error.clone().map(|error| {
                        div()
                            .id("mention-error")
                            .a11y_status(error.clone(), false)
                            .px_2()
                            .py_1()
                            .text_sm()
                            .text_color(colors.muted_foreground)
                            .child(error)
                    }))
            });
        div()
            .id("native-note-editor")
            .size_full()
            .flex()
            .flex_col()
            .pt_2()
            .relative()
            .bg(colors.card)
            .font_family(crate::ui::theme::system_font(cx))
            .text_color(colors.foreground)
            .track_focus(&self.focus)
            .role(gpui::Role::MultilineTextInput)
            .when_some(self.placeholder.clone(), |view, placeholder| {
                view.aria_placeholder(placeholder)
            })
            .when(window.is_a11y_active(), |view| {
                let (text, selection) = self
                    .model
                    .as_ref()
                    .map(|model| super::accessibility::plain_text(&model.document, model.selection))
                    .unwrap_or_default();
                let read_only = self.read_only;
                view.aria_value(text.clone())
                    .a11y_synthetic_children(move |builder| {
                        if read_only {
                            builder.parent_node().set_read_only();
                        }
                        crate::ui::a11y::text_runs(builder, &text, selection);
                    })
            })
            .on_key_down(cx.listener(Self::key))
            .on_drag_move(cx.listener(
                |this, event: &gpui::DragMoveEvent<super::drop::SessionDrag>, _, cx| {
                    this.drag_over(event.event.position, event.bounds, false, cx)
                },
            ))
            .on_drag_move(cx.listener(
                |this, event: &gpui::DragMoveEvent<gpui::ExternalPaths>, _, cx| {
                    let declined = super::drop::note_audio_drop(event.drag(cx).paths());
                    this.drag_over(event.event.position, event.bounds, declined, cx)
                },
            ))
            .when(!self.read_only, |view| {
                view.on_drop(
                    cx.listener(|this, drag: &super::drop::SessionDrag, window, cx| {
                        this.drop_session(drag, window, cx)
                    }),
                )
            })
            // A single audio file is left to the note, which takes it as the recording.
            .when(!self.read_only && !self.drop_declined, |view| {
                view.on_drop(
                    cx.listener(|this, paths: &gpui::ExternalPaths, window, cx| {
                        this.drop_files(paths.paths(), window, cx)
                    }),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
                    // Shipping's chips are buttons: a click opens the link and leaves the caret.
                    if let Some(position) =
                        super::app_link::chip_at(this.layouts.values(), event.position)
                    {
                        this.activate_at(position, cx);
                        cx.stop_propagation();
                        return;
                    }
                    this.selected_image = None;
                    this.focus.focus(window, cx);
                    Caret::reset_entity(&this.caret, cx);
                    if let Some(position) = this.hit(event.position) {
                        if event.modifiers.secondary(cx) {
                            this.activate_at(position, cx);
                            return;
                        }
                        if let Some(model) = &mut this.model {
                            match event.click_count {
                                2 => {
                                    let _ = model.select_word_at(position);
                                }
                                3.. => {
                                    let _ = model.select_block_at(position);
                                }
                                _ => {
                                    model.select(Selection {
                                        anchor: if event.modifiers.shift {
                                            model.selection.anchor
                                        } else {
                                            position
                                        },
                                        head: position,
                                    });
                                    this.dragging = true;
                                }
                            }
                        }
                    }
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if !event
                    .pressed_button
                    .is_some_and(|button| button == MouseButton::Left)
                {
                    this.dragging = false;
                    this.drag_scroll = None;
                }
                if this.dragging
                    && let Some(position) = this.hit(event.position)
                    && let Some(model) = &mut this.model
                {
                    model.select(Selection {
                        anchor: model.selection.anchor,
                        head: position,
                    });
                    this.drag_position = Some(event.position);
                    this.start_drag_scroll(cx);
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| {
                    this.dragging = false;
                    this.drag_scroll = None;
                }),
            )
            .when(!self.message.is_empty(), |container| {
                container.child(div().p_2().text_sm().child(self.message.clone()))
            })
            .when(self.pending_attachment.is_some(), |view| {
                view.child(
                    div()
                        .id("insert-upload")
                        .px_3()
                        .py_1()
                        .text_sm()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.insert_pending_attachment(cx)))
                        .child("Insert uploaded attachment at cursor"),
                )
            })
            .when(
                !self.read_only
                    && self
                        .model
                        .as_ref()
                        .is_some_and(|model| !model.selection.is_empty()),
                |view| {
                    let active = |kind: &str| {
                        self.model
                            .as_ref()
                            .is_some_and(|model| model.mark_active(kind))
                    };
                    view.child(gpui::deferred(
                        div()
                            .id("format-toolbar")
                            .a11y_group(gpui::Role::Toolbar, "Format selection")
                            .absolute()
                            .left(toolbar_position.x)
                            .top(toolbar_position.y)
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .p_1()
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.popover)
                            .shadow_lg()
                            .children(
                                [
                                    ("bold", "editor/TextBoldIcon.svg", "Bold"),
                                    ("italic", "editor/TextItalicIcon.svg", "Italic"),
                                    ("underline", "editor/TextUnderlineIcon.svg", "Underline"),
                                    (
                                        "strike",
                                        "editor/TextStrikethroughIcon.svg",
                                        "Strikethrough",
                                    ),
                                    ("code", "editor/CodeIcon.svg", "Code"),
                                    ("highlight", "editor/HighlighterIcon.svg", "Highlight"),
                                ]
                                .into_iter()
                                .map(|(kind, icon, label)| {
                                    let on = active(kind);
                                    div()
                                        .id(kind)
                                        // Shipping's toggle buttons (`aria-pressed`).
                                        .a11y_button(label)
                                        .aria_toggled(crate::ui::a11y::toggled(on))
                                        .a11y_press(cx, move |this, _, cx| {
                                            if let Some(model) = &mut this.model {
                                                let before = model.revision;
                                                let result = model.toggle_mark(kind);
                                                this.edited(before, result, cx);
                                            }
                                        })
                                        .group(kind)
                                        .size(px(28.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor_pointer()
                                        .when_else(
                                            on,
                                            |view| view.bg(colors.primary),
                                            |view| view.hover(|style| style.bg(colors.accent)),
                                        )
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |this, _, _, cx| {
                                                if let Some(model) = &mut this.model {
                                                    let before = model.revision;
                                                    let result = model.toggle_mark(kind);
                                                    this.edited(before, result, cx);
                                                }
                                                cx.stop_propagation();
                                            }),
                                        )
                                        .child(
                                            gpui::svg()
                                                .path(icon)
                                                .size(px(16.))
                                                .text_color(if on {
                                                    colors.primary_foreground
                                                } else {
                                                    colors.muted_foreground
                                                })
                                                .when(!on, |svg| {
                                                    svg.group_hover(kind, |style| {
                                                        style.text_color(colors.accent_foreground)
                                                    })
                                                }),
                                        )
                                }),
                            ),
                    ))
                },
            )
            .when_some(self.link_input.clone(), |view, input| {
                view.child(
                    div()
                        .id("link-editor")
                        .a11y_group(gpui::Role::Group, "Link")
                        .aria_value(input.clone())
                        .flex()
                        .gap_2()
                        .p_2()
                        .border_1()
                        .border_color(colors.border)
                        .child(if input.is_empty() {
                            "Enter URL…".into()
                        } else {
                            input
                        })
                        .child(
                            div()
                                .id("save-link")
                                .a11y_button("Apply")
                                .a11y_press(cx, |this, _, cx| this.commit_link(cx))
                                .cursor_pointer()
                                .child("Apply")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.commit_link(cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        )
                        .child(
                            div()
                                .id("remove-link")
                                .a11y_button("Remove")
                                .a11y_press(cx, |this, _, cx| {
                                    this.link_input = Some(String::new());
                                    this.commit_link(cx);
                                })
                                .cursor_pointer()
                                .child("Remove")
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _, _, cx| {
                                        this.link_input = Some(String::new());
                                        this.commit_link(cx);
                                        cx.stop_propagation();
                                    }),
                                ),
                        ),
                )
            })
            // The note's conflict banner owns a save conflict; these rows cover everything else.
            .when(self.save_failed && !self.save_conflict(), |container| {
                container.child(
                    div()
                        .id("retry-editor-save")
                        .a11y_button("Unsaved · Retry save")
                        .text_sm()
                        .px_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.retry_save(cx)))
                        .child("Unsaved · Retry save"),
                )
            })
            .when(
                self.external.is_some() && !self.save_conflict(),
                |container| {
                    container.child(
                        div()
                            .id("merge-editor-remote")
                            .a11y_button("Merge nonoverlapping remote changes")
                            .text_sm()
                            .px_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| this.merge_external(cx)))
                            .child("Merge nonoverlapping remote changes"),
                    )
                },
            )
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(
                        list(self.list.clone(), move |index, window, cx| {
                            entity.update(cx, |this, cx| this.render_block(index, window, cx))
                        })
                        .size_full(),
                    )
                    .child(
                        canvas(
                            move |_, _, _| (),
                            move |bounds, (), window, cx| {
                                let resize_entity = viewport_entity.clone();
                                viewport_entity.update(cx, |this, cx| {
                                    this.viewport = Some(bounds);
                                    this.finish_reveal(window, cx);
                                    this.finish_find_reveal(window, cx);
                                    this.paint_drop_cursor(window);
                                    // Shipping follows the pointer anywhere in the window
                                    // until release.
                                    if this.resize.is_some() {
                                        let moved = resize_entity.clone();
                                        window.on_mouse_event(
                                            move |event: &gpui::MouseMoveEvent, phase, _, cx| {
                                                if phase == gpui::DispatchPhase::Bubble {
                                                    moved.update(cx, |this, cx| {
                                                        this.resize_move(event.position.x, cx)
                                                    });
                                                }
                                            },
                                        );
                                        window.on_mouse_event(
                                            move |_: &gpui::MouseUpEvent, phase, _, cx| {
                                                if phase == gpui::DispatchPhase::Bubble {
                                                    resize_entity.update(cx, |this, cx| {
                                                        this.resize_commit(cx)
                                                    });
                                                }
                                            },
                                        );
                                    }
                                });
                            },
                        )
                        .absolute()
                        .inset_0(),
                    ),
            )
            .children(menu.map(gpui::deferred))
            .children(mentions.map(gpui::deferred))
            .child(
                canvas(
                    move |_, _, _| (),
                    move |bounds, (), window, cx| {
                        let focus = handler.read(cx).focus.clone();
                        window.handle_input(
                            &focus,
                            crate::zoom::ZoomedInput(ElementInputHandler::new(bounds, handler)),
                            cx,
                        );
                    },
                )
                .absolute()
                .size_full()
                .top_0()
                .left_0(),
            )
    }
}

impl Drop for EditorPane {
    fn drop(&mut self) {
        self.watch_cancel.cancel();
        if let Some(cancel) = &self.mention_cancel {
            cancel.cancel();
        }
    }
}

/// The node an imported attachment becomes: an image for images, otherwise a file card.
fn attachment_node(
    preview: &super::AttachmentPreview,
) -> (&'static str, serde_json::Map<String, serde_json::Value>) {
    let attrs = serde_json::json!({
        "attachmentId": preview.id, "name": preview.name, "alt": preview.name,
        "mimeType": preview.mime, "size": preview.size
    })
    .as_object()
    .expect("attrs")
    .clone();
    let kind = if preview.mime.starts_with("image/") {
        "image"
    } else {
        "fileAttachment"
    };
    (kind, attrs)
}

fn with_title(body: Arc<str>, title: Option<&str>) -> Arc<str> {
    title
        .and_then(|title| super::title::ensure_first_line_title(&body, title))
        .map_or(body, Into::into)
}
