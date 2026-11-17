//! Requests from standalone note windows, which the main window carries out.

use desktop_runtime::SessionId;
use gpui::{Context, Entity, Subscription, Window};

use super::ApplicationView;
use crate::{
    contracts::{EditorEvent, WorkspaceEvent},
    editor::menu::EditorRequest,
    note_window::NoteWindow,
    workspace::{NoteEvent, navigation::Route},
};

impl ApplicationView {
    pub(super) fn note_window_subscriptions(
        &mut self,
        view: &Entity<NoteWindow>,
        cx: &mut Context<Self>,
    ) -> Vec<Subscription> {
        vec![
            cx.subscribe(view, |this, _, event: &EditorEvent, cx| match event {
                EditorEvent::OpenLink(url)
                    if url.starts_with("https://")
                        || url.starts_with("http://")
                        || url.starts_with("mailto:") =>
                {
                    cx.open_url(url)
                }
                EditorEvent::MentionHuman(id) => {
                    let route = Route::Human(id.0.clone());
                    this.in_main_window(true, cx, move |this, _, cx| {
                        this.workspace
                            .update(cx, |view, cx| view.open_route(route, false, cx))
                    })
                }
                _ => {}
            }),
            cx.subscribe(view, |this, _, event: &EditorRequest, cx| match event {
                EditorRequest::OpenSession(id) => {
                    this.open_note_window(Some(id.clone().into()), cx)
                }
                EditorRequest::OpenOrganization(id) => {
                    let route = Route::Organization(id.clone().into());
                    this.in_main_window(true, cx, move |this, _, cx| {
                        this.workspace
                            .update(cx, |view, cx| view.open_route(route, false, cx))
                    })
                }
                EditorRequest::MentionSearch { .. } => {}
            }),
            cx.subscribe(view, |this, _, event: &WorkspaceEvent, cx| {
                // Recording runs in the background; product surfaces open in the main window.
                if matches!(
                    event,
                    WorkspaceEvent::Meeting(_) | WorkspaceEvent::Product(_)
                ) {
                    let activate = matches!(event, WorkspaceEvent::Product(_));
                    let event = event.clone();
                    this.in_main_window(activate, cx, move |this, window, cx| {
                        this.workspace_event(&event, window, cx)
                    })
                }
            }),
            cx.subscribe(view, |this, _, event: &NoteEvent, cx| match event {
                NoteEvent::Request(request) => {
                    let request = request.clone();
                    this.in_main_window(true, cx, move |this, _, cx| {
                        this.workspace
                            .update(cx, |view, cx| view.handle_note(&request, cx))
                    })
                }
                NoteEvent::OpenTemplates => this.in_main_window(true, cx, |this, _, cx| {
                    this.workspace
                        .update(cx, |view, cx| view.open_route(Route::Templates, false, cx))
                }),
                _ => {}
            }),
        ]
    }

    /// A deleted note's window has nothing left to show or save.
    pub(super) fn close_note_windows(&mut self, ids: &[SessionId], cx: &mut Context<Self>) {
        for id in ids {
            self.note_subscriptions.remove(id);
            if let Some(window) = self.note_windows.remove(id) {
                let _ = window.update(cx, |_, window, _| window.remove_window());
            }
        }
    }

    /// Runs `action` in the main window once the current update ends, optionally bringing
    /// that window to the front.
    fn in_main_window(
        &self,
        activate: bool,
        cx: &mut Context<Self>,
        action: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        let (main, view) = (self.window, cx.entity());
        cx.defer(move |cx| {
            let _ = main.update(cx, |_, window, cx| {
                if activate {
                    window.activate_window();
                }
                view.update(cx, |this, cx| action(this, window, cx))
            });
        });
    }
}
