//! Shipping's note Export modal (`outer-header/overflow/export-modal.tsx`): PDF, TXT, Markdown or
//! Org with the memo, the shown summary and the transcript, written into the export folder
//! (Downloads by default) and revealed there.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use desktop_runtime::{DocumentId, Result, RuntimeHandle, SessionId};
use gpui::{
    App, BoxShadow, Context, EventEmitter, FocusHandle, Global, Hsla, Render, SharedString,
    WeakEntity, Window, div, hsla, point, prelude::*, px, rgba,
};
use serde_json::{Value, json};

use crate::ui::a11y::Accessible;
use crate::{
    editor::EditorPane,
    product::{
        local::{
            data,
            export::{self, ExportOptions, Format},
        },
        settings::{self, Snapshot},
    },
    ui::theme::{MEDIUM, Theme, theme},
};

/// Shows a finished export; tests install a recorder instead of Finder.
#[derive(Clone)]
pub struct RevealExport(pub Arc<dyn Fn(&Path) + Send + Sync>);

impl Global for RevealExport {}

fn reveal(path: &Path, cx: &mut App) {
    match cx.try_global::<RevealExport>() {
        Some(reveal) => (reveal.0.clone())(path),
        None if crate::platform::os::headless() => {}
        None => cx.reveal_path(path),
    }
}

const FORMATS: [(Format, &str); 4] = [
    (Format::Pdf, "PDF"),
    (Format::Text, "TXT"),
    (Format::Markdown, "Markdown"),
    (Format::Org, "Org"),
];

fn format_key(format: Format) -> &'static str {
    format.extension()
}

fn parse_format(value: &str) -> Format {
    FORMATS
        .iter()
        .map(|(format, _)| *format)
        .find(|format| format_key(*format) == value)
        .unwrap_or(Format::Pdf)
}

/// Shipping's file name: `<title>_<ISO time with : and . as ->.<ext>`.
pub(super) fn file_name(title: &str, now: chrono::DateTime<chrono::Utc>, format: Format) -> String {
    let title = title.trim();
    let title = if title.is_empty() { "Untitled" } else { title };
    let safe: String = title
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let stamp = now
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        .replace([':', '.'], "-");
    format!("{safe}_{stamp}.{}", format.extension())
}

pub enum ExportEvent {
    Close,
}

pub struct ExportModal {
    runtime: RuntimeHandle,
    session: SessionId,
    title: Arc<str>,
    /// The summary the note shows; shipping exports only that one, and none from other views.
    summary_id: Option<DocumentId>,
    format: Format,
    memo: bool,
    summary: bool,
    transcript: bool,
    settings: Option<Snapshot>,
    pending: bool,
    failed: bool,
    focus: FocusHandle,
    /// The note's open editors, saved before reading the note so recent typing is exported.
    editors: Vec<WeakEntity<EditorPane>>,
}

impl EventEmitter<ExportEvent> for ExportModal {}

impl ExportModal {
    pub fn new(
        runtime: RuntimeHandle,
        session: SessionId,
        title: Arc<str>,
        summary_id: Option<DocumentId>,
        editors: Vec<WeakEntity<EditorPane>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let reply = settings::watch(&runtime);
        cx.spawn(async move |this, cx| {
            let Ok(watch) = async { reply?.receive().await }.await else {
                return;
            };
            let rows = watch.snapshots.borrow().rows.clone();
            let _ = watch.unsubscribe().await;
            if let Ok(snapshot) = settings::decode(&rows) {
                let _ = this.update(cx, |this, cx| {
                    let value = |key: &str| snapshot.get(key).map(|setting| setting.value.clone());
                    this.format = parse_format(
                        value("export_format")
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .as_deref()
                            .unwrap_or("pdf"),
                    );
                    this.memo = value("export_include_memo")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    this.summary = value("export_include_summary")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    this.transcript = value("export_include_transcript")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    this.settings = Some(snapshot);
                    cx.notify();
                });
            }
        })
        .detach();
        Self {
            runtime,
            session,
            title,
            summary_id,
            format: Format::Pdf,
            memo: false,
            summary: true,
            transcript: false,
            settings: None,
            pending: false,
            failed: false,
            focus,
            editors,
        }
    }

    fn directory(&self) -> Option<PathBuf> {
        let stored = self
            .settings
            .as_ref()
            .and_then(|snapshot| snapshot.get("export_directory"))
            .and_then(|setting| setting.value.as_str())
            .unwrap_or_default();
        if stored.is_empty() {
            std::env::home_dir().map(|home| home.join("Downloads"))
        } else {
            Some(PathBuf::from(stored))
        }
    }

    pub(super) fn export(&mut self, cx: &mut Context<Self>) {
        if self.pending || !(self.memo || self.summary || self.transcript) {
            return;
        }
        let Some(directory) = self.directory() else {
            self.failed = true;
            cx.notify();
            return;
        };
        let options = ExportOptions {
            format: self.format,
            memo: self.memo,
            summary: self.summary && self.summary_id.is_some(),
            transcript: self.transcript,
            summary_id: self.summary_id.as_ref().map(|id| id.0.to_string()),
        };
        let path = directory.join(file_name(&self.title, chrono::Utc::now(), self.format));
        let submitted = [
            ("export_format", json!(format_key(self.format))),
            ("export_include_memo", json!(self.memo)),
            ("export_include_summary", json!(self.summary)),
            ("export_include_transcript", json!(self.transcript)),
        ];
        self.pending = true;
        self.failed = false;
        cx.notify();
        let flushes: Vec<_> = self
            .editors
            .iter()
            .filter_map(|editor| editor.upgrade())
            .map(|editor| editor.update(cx, |editor, _| editor.flush()))
            .collect();
        let runtime = self.runtime.clone();
        let session = self.session.clone();
        cx.spawn(async move |this, cx| {
            // A failed save keeps its draft in the editor; the stored note still exports.
            for flush in flushes {
                let _ = flush.await;
            }
            let target = path.clone();
            let result: Result<()> = async {
                let meeting = data::snapshot(&runtime, session).await?;
                cx.background_executor()
                    .spawn(async move { export::export(&meeting, &options, &target) })
                    .await
            }
            .await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                match result {
                    Ok(()) => {
                        this.persist(&submitted);
                        reveal(&path, cx);
                        cx.emit(ExportEvent::Close);
                    }
                    Err(error) => {
                        tracing::warn!(%error, "note export failed");
                        this.failed = true;
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Remembers the choices like shipping; a lost race with another writer only skips that.
    fn persist(&self, submitted: &[(&'static str, Value)]) {
        let Some(snapshot) = &self.settings else {
            return;
        };
        for (key, value) in submitted {
            if let Some(base) = snapshot.get(*key).filter(|base| &base.value != value) {
                let _ = settings::save(&self.runtime, key.to_string(), base.clone(), value.clone());
            }
        }
    }

    fn radio(
        &self,
        format: Format,
        label: &'static str,
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let checked = self.format == format;
        div()
            .id(SharedString::from(format!(
                "export-format-{}",
                format.extension()
            )))
            .a11y_radio(label, checked)
            .flex()
            .items_center()
            .gap(px(6.))
            .text_sm()
            .line_height(px(20.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.format = format;
                cx.notify();
            }))
            .child(
                div()
                    .size(px(13.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_1()
                    .border_color(if checked {
                        colors.primary
                    } else {
                        control_border()
                    })
                    .bg(colors.background)
                    .when(checked, |control| {
                        control.child(div().size(px(7.)).bg(colors.primary))
                    }),
            )
            .child(label)
    }

    fn checkbox(
        &self,
        id: &'static str,
        label: &'static str,
        checked: bool,
        toggle: fn(&mut Self),
        colors: Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        div()
            .id(id)
            .a11y_checkbox(label, checked)
            .flex()
            .items_center()
            .gap(px(6.))
            .text_sm()
            .line_height(px(20.))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                toggle(this);
                cx.notify();
            }))
            .child(
                div()
                    .size(px(13.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .map(|control| {
                        if checked {
                            control.bg(colors.primary).child(
                                gpui::svg()
                                    .path("editor/CheckIcon.svg")
                                    .size(px(11.))
                                    .text_color(colors.primary_foreground),
                            )
                        } else {
                            control
                                .border_1()
                                .border_color(control_border())
                                .bg(colors.background)
                        }
                    }),
            )
            .child(label)
    }
}

/// Chromium's unchecked radio and checkbox outline.
fn control_border() -> Hsla {
    rgba(0x767676ff).into()
}

impl Render for ExportModal {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(window);
        let selected = self.memo || self.summary || self.transcript;
        let disabled = self.pending || !selected;
        let section = |title: &'static str| {
            div()
                .text_sm()
                .line_height(px(20.))
                .font_weight(MEDIUM)
                .child(title)
        };
        let card = div()
            .id("export-modal")
            .w(px(310.))
            .occlude()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .border_1()
            .border_color(colors.border.opacity(0.8))
            .bg(colors.background)
            .text_color(colors.foreground)
            .shadow(vec![BoxShadow {
                color: hsla(0., 0., 0., 0.25),
                offset: point(px(0.), px(25.)),
                blur_radius: px(50.),
                spread_radius: px(-12.),
                inset: false,
            }])
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_base()
                            .line_height(px(24.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Export"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .line_height(px(20.))
                            .text_color(colors.muted_foreground)
                            .child("Choose a file format and what to include."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(section("File format"))
                            .child(div().id("export-formats").a11y_group(gpui::Role::RadioGroup, "File format").flex().justify_center().gap_4().children(
                                FORMATS
                                    .iter()
                                    .map(|(format, label)| self.radio(*format, label, colors, cx))
                                    .collect::<Vec<_>>(),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_2()
                            .child(section("Include"))
                            .child(
                                div()
                                    .id("export-include")
                                    .a11y_group(gpui::Role::Group, "Include")
                                    .flex()
                                    .justify_center()
                                    .gap_4()
                                    .child(self.checkbox(
                                        "export-memo",
                                        "Memo",
                                        self.memo,
                                        |this| this.memo = !this.memo,
                                        colors,
                                        cx,
                                    ))
                                    .child(self.checkbox(
                                        "export-summary",
                                        "Summary",
                                        self.summary,
                                        |this| this.summary = !this.summary,
                                        colors,
                                        cx,
                                    ))
                                    .child(self.checkbox(
                                        "export-transcript",
                                        "Transcript",
                                        self.transcript,
                                        |this| this.transcript = !this.transcript,
                                        colors,
                                        cx,
                                    )),
                            ),
                    ),
            )
            .when(self.failed, |card| {
                card.child(
                    div()
                        .id("export-error")
                        .a11y_status(
                            "Could not export. Check the export location in Settings and try again.",
                            true,
                        )
                        .text_xs()
                        .text_center()
                        .text_color(rgba(0xef4444ff))
                        .child("Could not export. Check the export location in Settings and try again."),
                )
            })
            .child(
                div()
                    .id("export-submit")
                    .a11y_button(if self.pending { "Exporting..." } else { "Export" })
                    .a11y_disabled(disabled)
                    .h(px(40.))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .border_2()
                    .border_color(colors.primary)
                    .bg(colors.primary)
                    .text_color(colors.primary_foreground)
                    .text_sm()
                    .font_weight(MEDIUM)
                    .shadow(vec![BoxShadow {
                        color: hsla(30. / 360., 0.05, 0.33, 0.4),
                        offset: point(px(0.), px(4.)),
                        blur_radius: px(14.),
                        spread_radius: px(0.),
                        inset: false,
                    }])
                    .when_else(
                        disabled,
                        |button| button.opacity(0.5).cursor_not_allowed(),
                        |button| {
                            button
                                .cursor_pointer()
                                .hover(move |style| style.bg(colors.primary.opacity(0.9)))
                                .on_click(cx.listener(|this, _, _, cx| this.export(cx)))
                        },
                    )
                    .child(if self.pending { "Exporting..." } else { "Export" }),
            );
        gpui::deferred(
            gpui::anchored().position(point(px(0.), px(0.))).child(
                div()
                    .id("export-overlay")
                    .a11y_dialog("Export")
                    .aria_description("Choose a file format and what to include.")
                    .track_focus(&self.focus)
                    .w(window.viewport_size().width)
                    .h(window.viewport_size().height)
                    .occlude()
                    .bg(rgba(0x00000033))
                    .flex()
                    .items_center()
                    .justify_center()
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            if !this.pending {
                                cx.emit(ExportEvent::Close)
                            }
                        }),
                    )
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "escape" && !this.pending {
                            cx.emit(ExportEvent::Close);
                            cx.stop_propagation();
                        }
                    }))
                    .child(card),
            ),
        )
        .with_priority(2)
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, time::Duration};

    use super::*;

    #[gpui::test]
    fn export_writes_the_chosen_parts_into_the_export_folder_and_remembers_them(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.executor().allow_parking();
        let directory = tempfile::tempdir().unwrap();
        let exports = tempfile::tempdir().unwrap();
        let (runtime, ready) = RuntimeHandle::start(desktop_runtime::Profile {
            database: directory.path().join("library.sqlite"),
        })
        .unwrap();
        futures::executor::block_on(ready.receive()).unwrap();
        let folder = exports.path().to_string_lossy().into_owned();
        futures::executor::block_on(async {
            runtime
                .submit(move |services| async move {
                    for (sql, params) in [
                        ("INSERT INTO sessions(id,title) VALUES ('s','Sync')", vec![]),
                        ("INSERT INTO app_settings(id,value_json) VALUES ('export_directory',?)", vec![json!(json!(folder).to_string())]),
                        (r#"INSERT INTO session_documents(id,session_id,kind,body_format,body) VALUES
                            ('s','s','note','prosemirror_json','{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Memo line"}]}]}'),
                            ('shown','s','summary','markdown','## Shown summary'),
                            ('other','s','summary','markdown','## Other summary')"#, vec![]),
                    ] {
                        services
                            .executor
                            .execute(sql.into(), params)
                            .await
                            .map_err(crate::workspace::mutations::failure)?;
                    }
                    Ok(())
                })
                .unwrap()
                .receive()
                .await
                .unwrap()
        });
        let revealed = Arc::new(Mutex::new(None::<PathBuf>));
        let record = revealed.clone();
        cx.update(|cx| {
            cx.set_global(RevealExport(Arc::new(move |path| {
                *record.lock().unwrap() = Some(path.to_owned())
            })))
        });
        let window = cx.add_window(|window, cx| {
            ExportModal::new(
                runtime.clone(),
                SessionId("s".into()),
                "Sync".into(),
                Some(DocumentId("shown".into())),
                Vec::new(),
                window,
                cx,
            )
        });
        let settle = |cx: &mut gpui::TestAppContext,
                      done: &dyn Fn(&mut gpui::TestAppContext) -> bool| {
            for _ in 0..400 {
                cx.run_until_parked();
                if done(cx) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            panic!("export did not settle");
        };
        settle(cx, &|cx| {
            window
                .read_with(cx, |modal, _| modal.settings.is_some())
                .unwrap()
        });
        window
            .update(cx, |modal, _, cx| {
                modal.format = Format::Markdown;
                modal.memo = true;
                modal.export(cx);
            })
            .unwrap();
        settle(cx, &|_| revealed.lock().unwrap().is_some());
        let path = revealed.lock().unwrap().clone().unwrap();
        assert_eq!(path.parent(), Some(exports.path()));
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("## Memo\nMemo line"));
        assert!(text.contains("## Summary\n## Shown summary"));
        assert!(!text.contains("Other summary"));
        let stored = futures::executor::block_on(async {
            let watch = settings::watch(&runtime).unwrap().receive().await.unwrap();
            let snapshot = settings::decode(&watch.snapshots.borrow().rows).unwrap();
            watch.unsubscribe().await.unwrap();
            snapshot
        });
        assert_eq!(stored["export_format"].value, json!("md"));
        assert_eq!(stored["export_include_memo"].value, json!(true));
        futures::executor::block_on(runtime.shutdown()).unwrap();
    }

    #[test]
    fn export_file_names_follow_shipping() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-11-17T08:51:28.123Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(
            file_name("Q4: plan/notes?", now, Format::Markdown),
            "Q4_ plan_notes__2026-11-17T08-51-28-123Z.md"
        );
        assert_eq!(
            file_name("  ", now, Format::Pdf),
            "Untitled_2026-11-17T08-51-28-123Z.pdf"
        );
    }
}
