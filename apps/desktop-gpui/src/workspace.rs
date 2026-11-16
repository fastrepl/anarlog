use std::path::PathBuf;

use gpui::{
    Context, Entity, FocusHandle, Focusable, Role, ScrollHandle, SharedString, Subscription, Task,
    UniformListScrollHandle, Window, actions, div, prelude::*, px, rgb, text, uniform_list,
};

use crate::{
    library::{Library, PAGE_SIZE, Session},
    text_input::{Changed, TextInput},
};

actions!(
    library,
    [
        Quit,
        OpenLibrary,
        Reload,
        Next,
        Previous,
        NextPage,
        PreviousPage,
        FocusFilter,
        Dismiss
    ]
);

pub struct Workspace {
    database: Option<PathBuf>,
    runtime: tokio::runtime::Handle,
    library: Option<Library>,
    sessions: Vec<Session>,
    selected: Option<usize>,
    offset: usize,
    has_more: bool,
    loading: bool,
    status: SharedString,
    note: SharedString,
    focus: FocusHandle,
    filter: Entity<TextInput>,
    _filter_changed: Subscription,
    list_scroll: UniformListScrollHandle,
    note_scroll: ScrollHandle,
    load_task: Option<Task<()>>,
    note_task: Option<Task<()>>,
    picker_task: Option<Task<()>>,
    load_abort: Option<tokio::task::AbortHandle>,
    note_abort: Option<tokio::task::AbortHandle>,
}

impl Workspace {
    pub fn new(
        database: Option<PathBuf>,
        runtime: tokio::runtime::Handle,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let filter = cx.new(|cx| TextInput::new("filter", "Filter by title", cx));
        let filter_changed = cx.subscribe(&filter, |this, _, _: &Changed, cx| {
            this.offset = 0;
            this.reload(cx);
        });
        let mut workspace = Self {
            database,
            runtime,
            library: None,
            sessions: Vec::new(),
            selected: None,
            offset: 0,
            has_more: false,
            loading: false,
            status: "No library selected".into(),
            note: "Choose Open Library to browse an existing app.db.\n\nThis native preview opens the database read-only. Notes appear as Markdown; editing and recording are not available yet.".into(),
            focus,
            filter,
            _filter_changed: filter_changed,
            list_scroll: UniformListScrollHandle::new(),
            note_scroll: ScrollHandle::new(),
            load_task: None,
            note_task: None,
            picker_task: None,
            load_abort: None,
            note_abort: None,
        };
        workspace.reload(cx);
        workspace
    }

    fn open_library(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open SQLite library read-only".into()),
        });
        self.picker_task = Some(cx.spawn(async move |this, cx| {
            let result = paths
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = this.update(cx, |this, cx| match result {
                Ok(Some(paths)) => {
                    if let Some(path) = paths.into_iter().next() {
                        this.database = Some(path);
                        this.library = None;
                        this.offset = 0;
                        this.reload(cx);
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    this.note = format!("Could not open the file picker: {error:#}").into();
                    cx.notify();
                }
            });
        }));
    }

    fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.database.clone() else {
            return;
        };
        self.note_task = None;
        if let Some(job) = self.note_abort.take() {
            job.abort();
        }
        if let Some(job) = self.load_abort.take() {
            job.abort();
        }
        self.selected = None;
        self.sessions.clear();
        self.has_more = false;
        self.loading = true;
        self.status = "Loading library…".into();
        self.note = "Select a note to preview it.".into();
        let library = self.library.clone();
        let offset = self.offset;
        let filter = self.filter.read(cx).text().trim().to_owned();
        let filtered = !filter.is_empty();
        let job = self.runtime.spawn(async move {
            let library = match library {
                Some(library) => library,
                None => Library::open(&path).await?,
            };
            let (sessions, has_more) = library.sessions(offset, &filter).await?;
            anyhow::Ok((library, sessions, has_more))
        });
        self.load_abort = Some(job.abort_handle());
        self.load_task = Some(cx.spawn(async move |this, cx| {
            let result = job
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = this.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok((library, sessions, has_more)) => {
                        this.library = Some(library);
                        this.sessions = sessions;
                        this.has_more = has_more;
                        this.status = if this.sessions.is_empty() && filtered {
                            "No titles match".into()
                        } else if this.sessions.is_empty() {
                            "No notes on this page".into()
                        } else {
                            format!(
                                "Notes {}–{}",
                                this.offset + 1,
                                this.offset + this.sessions.len()
                            )
                            .into()
                        };
                        this.list_scroll
                            .scroll_to_item(0, gpui::ScrollStrategy::Top);
                        if !this.sessions.is_empty() {
                            this.select(0, cx);
                        }
                    }
                    Err(error) => {
                        this.status = "Could not load library".into();
                        this.note = format!("{error:#}").into();
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn select(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(session) = self.sessions.get(index) else {
            return;
        };
        let Some(library) = self.library.clone() else {
            return;
        };
        let id = session.id.clone();
        if let Some(job) = self.note_abort.take() {
            job.abort();
        }
        self.selected = Some(index);
        self.note = "Loading note…".into();
        self.note_scroll.set_offset(gpui::point(px(0.), px(0.)));
        self.list_scroll
            .scroll_to_item(index, gpui::ScrollStrategy::Center);
        let job = self.runtime.spawn(async move { library.note(&id).await });
        self.note_abort = Some(job.abort_handle());
        self.note_task = Some(cx.spawn(async move |this, cx| {
            let result = job.await.map_err(anyhow::Error::from).and_then(|result| result);
            let _ = this.update(cx, |this, cx| {
                this.note = match result {
                    Ok(note) if note.is_empty() => "This note is empty.".into(),
                    Ok(note) => {
                        let mut chars = note.chars();
                        let mut preview: String = chars.by_ref().take(30_000).collect();
                        if chars.next().is_some() {
                            preview.push_str("\n\nPreview limited to 30,000 characters. Open the current desktop app for the full note.");
                        }
                        preview.into()
                    }
                    Err(error) => format!("{error:#}").into(),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn next_page(&mut self, cx: &mut Context<Self>) {
        if self.has_more && !self.loading {
            self.offset += PAGE_SIZE;
            self.reload(cx);
        }
    }

    fn previous_page(&mut self, cx: &mut Context<Self>) {
        if self.offset > 0 && !self.loading {
            self.offset = self.offset.saturating_sub(PAGE_SIZE);
            self.reload(cx);
        }
    }

    /// Escape clears the filter and returns the keyboard to the note list.
    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |filter, cx| {
            if !filter.text().is_empty() {
                filter.clear(window, cx);
            }
        });
        self.focus.focus(window, cx);
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        if let Some(job) = self.load_abort.take() {
            job.abort();
        }
        if let Some(job) = self.note_abort.take() {
            job.abort();
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title: SharedString = self
            .selected
            .and_then(|index| self.sessions.get(index))
            .map(|session| {
                if session.title.is_empty() {
                    "Untitled note"
                } else {
                    &session.title
                }
            })
            .unwrap_or("Your notes, natively")
            .to_owned()
            .into();
        div()
            .size_full()
            .flex()
            .bg(rgb(0xfafaf9))
            .text_color(rgb(0x292524))
            .key_context("Library")
            .on_action(cx.listener(|this, _: &FocusFilter, window, cx| {
                let filter = this.filter.focus_handle(cx);
                window.focus(&filter, cx);
            }))
            .on_action(cx.listener(|this, _: &Dismiss, window, cx| this.dismiss(window, cx)))
            .on_action(cx.listener(|this, _: &OpenLibrary, _, cx| this.open_library(cx)))
            .on_action(cx.listener(|this, _: &Reload, _, cx| this.reload(cx)))
            .on_action(cx.listener(|this, _: &NextPage, _, cx| this.next_page(cx)))
            .on_action(cx.listener(|this, _: &PreviousPage, _, cx| this.previous_page(cx)))
            .on_action(cx.listener(|this, _: &Next, _, cx| {
                this.select(this.selected.map_or(0, |index| index + 1), cx);
            }))
            .on_action(cx.listener(|this, _: &Previous, _, cx| {
                this.select(this.selected.unwrap_or(0).saturating_sub(1), cx);
            }))
            .child(
                div()
                    .w(px(300.))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .border_r_1()
                    .border_color(rgb(0xe7e5e4))
                    .child(
                        div().p_5().text_xl().child("Anarlog").child(
                            div()
                                .text_sm()
                                .text_color(rgb(0x78716c))
                                .child("Native preview · Read-only"),
                        ),
                    )
                    .child(
                        div()
                            .id("open-library")
                            .role(Role::Button)
                            .aria_label("Open Library")
                            .px_5()
                            .pb_3()
                            .text_sm()
                            .cursor_pointer()
                            .child("Open Library…")
                            .on_click(cx.listener(|this, _, _, cx| this.open_library(cx))),
                    )
                    .child(
                        div()
                            .mx_5()
                            .mb_3()
                            .px_2()
                            .py_1()
                            .text_sm()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0xd6d3d1))
                            .bg(rgb(0xffffff))
                            .child(self.filter.clone()),
                    )
                    .child(
                        div()
                            .id("status")
                            .role(Role::Status)
                            .aria_label(self.status.clone())
                            .px_5()
                            .pb_3()
                            .text_sm()
                            .child(self.status.clone()),
                    )
                    .child(
                        // The list keeps keyboard focus; the selected row is its active descendant.
                        div()
                            .id("notes")
                            .role(Role::ListBox)
                            .aria_label("Notes")
                            .track_focus(&self.focus)
                            .key_context("NoteList")
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .flex_col()
                            .child(
                                uniform_list(
                                    "sessions",
                                    self.sessions.len(),
                                    cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                                        range
                                            .map(|index| {
                                                let session = &this.sessions[index];
                                                let title = if session.title.is_empty() {
                                                    "Untitled note"
                                                } else {
                                                    &session.title
                                                };
                                                let date: String =
                                                    session.date.chars().take(10).collect();
                                                div()
                                                    .id(index)
                                                    .role(Role::ListBoxOption)
                                                    .aria_label(format!("{title}, {date}"))
                                                    .aria_selected(this.selected == Some(index))
                                                    .aria_position_in_set(this.offset + index + 1)
                                                    .h(px(64.))
                                                    .px_5()
                                                    .py_2()
                                                    .cursor_pointer()
                                                    .when(this.selected == Some(index), |row| {
                                                        row.aria_active_descendant()
                                                            .bg(rgb(0xe7e5e4))
                                                    })
                                                    .hover(|style| style.bg(rgb(0xf0efed)))
                                                    .on_click(cx.listener(
                                                        move |this, _, window, cx| {
                                                            this.focus.focus(window, cx);
                                                            this.select(index, cx);
                                                        },
                                                    ))
                                                    .child(
                                                        div()
                                                            .text_ellipsis()
                                                            .child(title.to_owned()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(rgb(0x78716c))
                                                            .child(date),
                                                    )
                                            })
                                            .collect::<Vec<_>>()
                                    }),
                                )
                                .track_scroll(&self.list_scroll)
                                .flex_1()
                                .min_h_0(),
                            ),
                    )
                    .child(
                        div()
                            .p_4()
                            .flex()
                            .gap_4()
                            .text_sm()
                            .child(
                                div()
                                    .id("previous")
                                    .role(Role::Button)
                                    .aria_label("Previous page")
                                    .cursor_pointer()
                                    .child("← Previous")
                                    .on_click(cx.listener(|this, _, _, cx| this.previous_page(cx))),
                            )
                            .child(
                                div()
                                    .id("next")
                                    .role(Role::Button)
                                    .aria_label("Next page")
                                    .cursor_pointer()
                                    .child("Next →")
                                    .on_click(cx.listener(|this, _, _, cx| this.next_page(cx))),
                            )
                            .child(
                                div()
                                    .id("reload")
                                    .role(Role::Button)
                                    .aria_label("Reload library")
                                    .cursor_pointer()
                                    .child("Reload")
                                    .on_click(cx.listener(|this, _, _, cx| this.reload(cx))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .px_8()
                            .py_6()
                            .border_b_1()
                            .border_color(rgb(0xe7e5e4))
                            .child(
                                div()
                                    .id("title")
                                    .role(Role::Heading)
                                    .aria_level(1)
                                    .aria_label(title.clone())
                                    .text_2xl()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .mt_2()
                                    .text_sm()
                                    .text_color(rgb(0x78716c))
                                    .child("Markdown preview"),
                            ),
                    )
                    .child(
                        div()
                            .id("note")
                            .role(Role::Document)
                            .aria_label("Note")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.note_scroll)
                            .p_8()
                            .child(text!(id = "body", self.note.clone())),
                    ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use super::*;

    #[gpui::test]
    fn filters_from_the_keyboard(cx: &mut TestAppContext) {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let handle = runtime.handle().clone();
        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::new(None, handle, window, cx));
        cx.update(|_, cx| cx.bind_keys(crate::key_bindings()));

        cx.simulate_keystrokes(secondary!("f"));
        cx.simulate_input("회의");
        workspace.update_in(cx, |workspace, window, cx| {
            assert_eq!(workspace.filter.read(cx).text(), "회의");
            assert!(workspace.filter.focus_handle(cx).is_focused(window));
        });

        cx.simulate_keystrokes("escape");
        workspace.update_in(cx, |workspace, window, cx| {
            assert_eq!(workspace.filter.read(cx).text(), "");
            assert!(workspace.focus.is_focused(window));
        });
    }
}
