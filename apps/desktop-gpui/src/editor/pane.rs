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
    menu::{
        BlockCommand, EditorRequest, MentionCandidate, MentionMenu, MentionResults, MentionTarget,
        SlashMenu,
    },
    model::{EditorModel, Selection},
    persistence::{Flush, SaveEvent, SaveJournal},
    surface::{self, ProjectedBlock, VisibleLayout},
};
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
                                this.message = format!("Unsaved changes: {error} Draft retained; retry or resolve the external revision.");
                                this.save_failed = true;
                                if matches!(error, ServiceError::Conflict) { this.fetch_conflict(cx); }
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
            open::that(preview.path.as_ref()).map_err(|error| error.to_string())
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
            let Some(file) = rfd::AsyncFileDialog::new().pick_file().await else {
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
        let image = preview.mime.starts_with("image/");
        let attrs = serde_json::json!({
            "attachmentId": preview.id, "name": preview.name, "alt": preview.name,
            "mimeType": preview.mime, "size": preview.size
        })
        .as_object()
        .expect("attrs")
        .clone();
        let before = model.revision;
        let result = model.insert_block_atom(if image { "image" } else { "fileAttachment" }, attrs);
        if result.is_ok() {
            self.pending_attachment = None;
        }
        self.edited(before, result, cx);
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
                view.child("Remove").on_mouse_down(
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

    fn render_attachment(
        &mut self,
        node: &super::document::NodeRef,
        start: usize,
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
            return Some(
                div()
                    .my_1()
                    .child(
                        gpui::img(src.to_owned())
                            .max_w_full()
                            .max_h(px(360.))
                            .when_some(
                                node.attr("editorWidth").and_then(serde_json::Value::as_f64),
                                |view, width| {
                                    view.w(gpui::DefiniteLength::Fraction(
                                        (width.clamp(10., 100.) / 100.) as f32,
                                    ))
                                },
                            )
                            .object_fit(gpui::ObjectFit::Contain),
                    )
                    .child(self.attachment_remove_control(start, node.id, cx))
                    .into_any_element(),
            );
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
            Some(Ok(preview)) => {
                let path = preview.path.clone();
                let image = preview.mime.starts_with("image/");
                Some(
                    div()
                        .id(("attachment", node.id))
                        .my_1()
                        .rounded_lg()
                        .border_1()
                        .px_3()
                        .py(px(10.))
                        .when(node.kind() == "fileAttachment", |view| {
                            view.flex().items_center().gap_3()
                        })
                        .cursor_pointer()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                let path = path.clone();
                                let job = cx.background_executor().spawn(async move {
                                    open::that(path.as_ref()).map_err(|error| error.to_string())
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
                                this.dragging = false;
                                cx.stop_propagation();
                            }),
                        )
                        .when(image, |view| {
                            view.child(
                                gpui::img(preview.path.as_ref().clone())
                                    .max_h(px(if node.kind() == "image" { 360. } else { 40. }))
                                    .max_w_full()
                                    .when(node.kind() == "fileAttachment", |view| {
                                        view.w(px(40.)).h(px(40.))
                                    })
                                    .when_some(
                                        node.attr("editorWidth")
                                            .and_then(serde_json::Value::as_f64)
                                            .filter(|_| node.kind() == "image"),
                                        |view, width| {
                                            view.w(gpui::DefiniteLength::Fraction(
                                                (width.clamp(10., 100.) / 100.) as f32,
                                            ))
                                        },
                                    )
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
        self.clipboard_generation += 1;
        let generation = self.clipboard_generation;
        let job = cx
            .background_executor()
            .spawn(async move { clipboard::copy(&document, selection) });
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
        let Some(model) = &self.model else {
            return;
        };
        let revision = model.revision;
        let selection = model.selection;
        let code = model
            .document
            .resolve(selection.head)
            .is_ok_and(|resolved| resolved.node.kind() == "codeBlock");
        self.clipboard_generation += 1;
        let generation = self.clipboard_generation;
        let job = cx.background_executor().spawn(async move {
            if let Some(metadata) = metadata { return clipboard::parse_slice(&metadata); }
            if rich && !code
                && let Ok(mut clipboard) = arboard::Clipboard::new()
                && clipboard.get_text().is_ok_and(|current| current == text)
                && let Ok(html) = clipboard.get().html()
            {
                return super::html::parse(&html);
            }
            if text.len() > 1024 * 1024 { return Err("Paste exceeds the 1 MiB transaction limit".into()); }
            let paragraphs = if code { vec![text.as_str()] } else { text.split('\n').collect() };
            let content: Vec<_> = paragraphs.into_iter().map(|line| serde_json::json!({
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
                let result = fragment.and_then(|(fragment, open)| {
                    if code {
                        let text = super::document::inline_text(&fragment.root.children.get(0).expect("paragraph").children);
                        model.replace(selection.range(), &text)
                    } else { model.insert_slice(fragment, open) }
                });
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
            self.message = "A remote revision is available. Your draft and composition are retained; resolve before saving.".into();
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
        if let Some(preview) = self.render_attachment(&node, start, cx) {
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
                        self.render_attachment(&span.node, start + row.start + span.position, cx)
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
                        .absolute()
                        .left(menu_position.x)
                        .top(menu_position.y)
                        .w(px(224.))
                        .max_h(px(256.))
                        .overflow_y_scroll()
                        .p_1()
                        .rounded(px(16.))
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
                                    .h(px(32.))
                                    .px_2()
                                    .flex()
                                    .items_center()
                                    .gap(px(6.))
                                    .rounded(px(12.))
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
                    .absolute()
                    .left(menu_position.x)
                    .top(menu_position.y)
                    .max_w(px(280.))
                    .min_w(px(160.))
                    .p_1()
                    .rounded(px(16.))
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
                                .h(px(32.))
                                .px_2()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .rounded(px(12.))
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
            .on_key_down(cx.listener(Self::key))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, window, cx| {
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
                            .absolute()
                            .left(toolbar_position.x)
                            .top(toolbar_position.y)
                            .flex()
                            .items_center()
                            .gap(px(2.))
                            .p_1()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(colors.border)
                            .bg(colors.popover)
                            .shadow_lg()
                            .children(
                                [
                                    ("bold", "editor/TextBoldIcon.svg"),
                                    ("italic", "editor/TextItalicIcon.svg"),
                                    ("underline", "editor/TextUnderlineIcon.svg"),
                                    ("strike", "editor/TextStrikethroughIcon.svg"),
                                    ("code", "editor/CodeIcon.svg"),
                                    ("highlight", "editor/HighlighterIcon.svg"),
                                ]
                                .into_iter()
                                .map(|(kind, icon)| {
                                    let on = active(kind);
                                    div()
                                        .id(kind)
                                        .group(kind)
                                        .size(px(28.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(6.))
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
            .when(self.save_failed, |container| {
                container.child(
                    div()
                        .id("retry-editor-save")
                        .text_sm()
                        .px_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.retry_save(cx)))
                        .child("Unsaved · Retry save"),
                )
            })
            .when(self.external.is_some(), |container| {
                container.child(
                    div()
                        .id("merge-editor-remote")
                        .text_sm()
                        .px_2()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.merge_external(cx)))
                        .child("Merge nonoverlapping remote changes"),
                )
            })
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
                                viewport_entity.update(cx, |this, cx| {
                                    this.viewport = Some(bounds);
                                    this.finish_reveal(window, cx);
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
                        window.handle_input(&focus, ElementInputHandler::new(bounds, handler), cx);
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

fn with_title(body: Arc<str>, title: Option<&str>) -> Arc<str> {
    title
        .and_then(|title| super::title::ensure_first_line_title(&body, title))
        .map_or(body, Into::into)
}
