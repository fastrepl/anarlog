//! The note editor driven through GPUI test input: find & replace, drops and image resizing.

use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Duration};

use desktop_runtime::{DocumentSnapshot, Profile, RuntimeHandle, SessionId};
use futures::executor::block_on;
use gpui::{
    Context, Entity, ExternalPaths, FileDropEvent, Modifiers, MouseButton, Render, TestAppContext,
    VisualTestContext, Window, div, point, prelude::*, px,
};
use serde_json::{Value, json};

use super::{
    EditorPane,
    document::{Document, inline_text},
    drop::SessionDrag,
    find::FindOptions,
    menu::EditorRequest,
};
use crate::contracts::{EditorInit, LaneContext};

/// The editor beside a draggable timeline note, inside a parent that records the drops the
/// editor leaves to it (as the note does for a recording).
struct Harness {
    editor: Entity<EditorPane>,
    passed: Rc<RefCell<Vec<PathBuf>>>,
}

impl Render for Harness {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let passed = self.passed.clone();
        div()
            .id("note")
            .size_full()
            .flex()
            .on_drop(move |paths: &ExternalPaths, _, _| {
                passed.borrow_mut().extend(paths.paths().iter().cloned())
            })
            .child(div().id("timeline-note").w(px(100.)).h(px(40.)).on_drag(
                SessionDrag::new(SessionId("weekly".into()), "Weekly sync"),
                |drag, _, _, cx| cx.new(|_| drag.clone()),
            ))
            .child(div().flex_1().h_full().child(self.editor.clone()))
    }
}

struct Fixture {
    harness: Entity<Harness>,
    editor: Entity<EditorPane>,
    directory: tempfile::TempDir,
    _runtime: RuntimeHandle,
}

fn paragraph(text: &str) -> Value {
    json!({"type": "paragraph", "content": [{"type": "text", "text": text}]})
}

fn fixture(cx: &mut TestAppContext, content: Vec<Value>) -> (Fixture, &mut VisualTestContext) {
    cx.executor().allow_parking();
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("library.sqlite"),
    })
    .unwrap();
    block_on(ready.receive()).unwrap();
    let open = block_on(runtime.create_note("Drops".into()).unwrap().receive()).unwrap();
    let session = open.summary.id.clone();
    let document: DocumentSnapshot = open.note.unwrap();
    let context = LaneContext {
        runtime: runtime.clone(),
    };
    let passed = Rc::default();
    cx.update(crate::ui::theme::UiFonts::init);
    let (harness, cx) = cx.add_window_view(|window, cx| {
        let init = EditorInit {
            session_id: session.clone(),
            document,
            return_focus: cx.focus_handle(),
        };
        let editor = cx.new(|cx| EditorPane::new(context, init, window, cx));
        Harness { editor, passed }
    });
    let editor = harness.read_with(cx, |harness, _| harness.editor.clone());
    settle(cx, |cx| {
        editor.read_with(cx, |editor, _| editor.model.is_some())
    });
    let fragment = Document::parse(
        json!({"type": "doc", "content": content})
            .to_string()
            .into(),
    );
    editor.update(cx, |editor, cx| {
        let model = editor.model.as_mut().unwrap();
        let before = model.revision;
        let result = model.replace_all(fragment.unwrap());
        editor.edited(before, result, cx);
    });
    frame(cx);
    let fixture = Fixture {
        harness,
        editor,
        directory,
        _runtime: runtime,
    };
    (fixture, cx)
}

fn frame(cx: &mut VisualTestContext) {
    cx.update(|window, cx| window.draw(cx).clear(cx));
}

/// Draws, then runs foreground work until `done`, letting the runtime's replies land.
fn settle(cx: &mut VisualTestContext, done: impl Fn(&mut VisualTestContext) -> bool) {
    for _ in 0..400 {
        frame(cx);
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    panic!("editor did not settle");
}

fn body(editor: &Entity<EditorPane>, cx: &mut VisualTestContext) -> Value {
    editor.read_with(cx, |editor, _| {
        serde_json::from_str(&editor.model.as_ref().unwrap().document.serialize().unwrap()).unwrap()
    })
}

fn texts(editor: &Entity<EditorPane>, cx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(cx, |editor, _| {
        let root = &editor.model.as_ref().unwrap().document.root;
        (0..root.children.len())
            .map(|index| inline_text(&root.children.get(index).unwrap().children))
            .collect()
    })
}

#[cfg(not(target_os = "macos"))]
#[gpui::test]
fn control_moves_and_deletes_one_word_in_notes(cx: &mut TestAppContext) {
    let (fixture, cx) = fixture(cx, vec![paragraph("one two three")]);
    let editor = fixture.editor;
    editor.update(cx, |editor, _| {
        editor
            .model
            .as_mut()
            .unwrap()
            .move_document_boundary(false, false);
    });
    cx.update(|window, cx| {
        editor.read(cx).focus_handle(cx).focus(window, cx);
        window.draw(cx).clear(cx);
    });
    for key in ["ctrl-right", "ctrl-right", "ctrl-left", "ctrl-delete"] {
        cx.update(|window, cx| {
            window.dispatch_keystroke(gpui::Keystroke::parse(key).unwrap(), cx);
        });
    }
    assert_eq!(texts(&editor, cx), ["one  three"]);
    cx.update(|window, cx| {
        window.dispatch_keystroke(gpui::Keystroke::parse("ctrl-backspace").unwrap(), cx);
    });
    assert_eq!(texts(&editor, cx), [" three"]);
}

#[gpui::test]
fn replace_moves_to_the_next_match_and_replace_all_is_one_undo(cx: &mut TestAppContext) {
    let (fixture, cx) = fixture(
        cx,
        vec![
            paragraph("one fish two fish"),
            paragraph("red Fish blue fish"),
        ],
    );
    let editor = fixture.editor.clone();
    editor.update(cx, |editor, cx| {
        editor.find("fish", FindOptions::default(), cx)
    });
    assert_eq!(editor.update(cx, |editor, _| editor.find_status()), (0, 4));
    editor.update(cx, |editor, cx| {
        editor.find_step(true, cx);
        editor.replace_found("cat", false, cx);
    });
    // The replaced match is gone and the next one ("Fish") is current.
    assert_eq!(editor.update(cx, |editor, _| editor.find_status()), (1, 3));
    assert_eq!(
        texts(&editor, cx),
        ["one fish two cat", "red Fish blue fish"]
    );
    editor.update(cx, |editor, cx| {
        editor.find(
            "fish",
            FindOptions {
                case_sensitive: true,
                whole_word: false,
            },
            cx,
        );
        editor.replace_found("dog", true, cx);
    });
    assert_eq!(texts(&editor, cx), ["one dog two cat", "red Fish blue dog"]);
    assert_eq!(editor.update(cx, |editor, _| editor.find_status()), (0, 0));
    editor.update(cx, |editor, _| {
        editor.model.as_mut().unwrap().undo().unwrap()
    });
    assert_eq!(
        texts(&editor, cx),
        ["one fish two cat", "red Fish blue fish"]
    );
}

#[gpui::test]
fn a_dragged_timeline_note_drops_into_the_note_as_a_session_mention(cx: &mut TestAppContext) {
    let (fixture, cx) = fixture(cx, vec![paragraph("Agenda")]);
    let editor = fixture.editor.clone();
    let target = point(px(400.), px(20.));
    frame(cx);
    cx.simulate_mouse_down(
        point(px(50.), px(20.)),
        MouseButton::Left,
        Modifiers::none(),
    );
    for position in [point(px(60.), px(20.)), target] {
        frame(cx);
        cx.simulate_mouse_move(position, MouseButton::Left, Modifiers::none());
    }
    frame(cx);
    assert!(editor.read_with(cx, |editor, _| editor.drop_cursor.is_some()));
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    let content = body(&editor, cx)["content"][0]["content"].clone();
    let mention = content
        .as_array()
        .unwrap()
        .iter()
        .position(|node| node["type"] == "mention-@")
        .expect("session mention");
    assert_eq!(
        content[mention]["attrs"],
        json!({"id": "weekly", "type": "session", "label": "Weekly sync"})
    );
    assert!(
        content[mention + 1]["text"]
            .as_str()
            .unwrap()
            .starts_with(' ')
    );
}

#[gpui::test]
fn dropped_files_attach_at_the_drop_but_a_lone_recording_goes_to_the_note(cx: &mut TestAppContext) {
    let (fixture, cx) = fixture(cx, vec![paragraph("Photos")]);
    let editor = fixture.editor.clone();
    let vault = fixture.directory.path().join("vault");
    editor.update(cx, |editor, cx| editor.configure_attachments(vault, cx));
    let drop = |cx: &mut VisualTestContext, paths: Vec<PathBuf>| {
        let position = point(px(400.), px(20.));
        frame(cx);
        cx.simulate_event(FileDropEvent::Entered {
            position,
            paths: ExternalPaths(paths.into()),
        });
        frame(cx);
        cx.simulate_event(FileDropEvent::Pending { position });
        frame(cx);
        cx.simulate_event(FileDropEvent::Submit { position });
    };

    let recording = fixture.directory.path().join("call.m4a");
    std::fs::write(&recording, b"audio").unwrap();
    drop(cx, vec![recording.clone()]);
    let passed = fixture
        .harness
        .read_with(cx, |harness, _| harness.passed.clone());
    assert_eq!(*passed.borrow(), std::slice::from_ref(&recording));

    let photo = fixture.directory.path().join("photo.png");
    std::fs::write(&photo, b"\x89PNG\r\n\x1a\n").unwrap();
    drop(cx, vec![photo.clone()]);
    settle(cx, |cx| {
        body(&editor, cx)["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["type"] == "image")
    });
    let image = body(&editor, cx)["content"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["type"] == "image")
        .cloned()
        .unwrap();
    assert_eq!(image["attrs"]["mimeType"], "image/png");
    assert!(
        image["attrs"]["attachmentId"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert_eq!(passed.borrow().len(), 1);

    // With other files, the recording still goes to the note's upload while they attach.
    let session = editor.read_with(cx, |editor, _| editor.init.session_id.clone());
    drop(cx, vec![photo, recording]);
    let uploads = cx.update(|_, cx| crate::meeting::upload::Uploads::global(cx));
    settle(cx, |cx| {
        uploads.read_with(cx, |uploads, _| uploads.job(&session).is_some())
    });
    assert_eq!(passed.borrow().len(), 1);
}

#[gpui::test]
fn image_resizing_keeps_shipping_limits_and_stores_a_percentage(cx: &mut TestAppContext) {
    let (fixture, cx) = fixture(
        cx,
        vec![
            json!({"type": "image", "attrs": {"src": "https://example.com/a.png"}}),
            json!({"type": "paragraph"}),
        ],
    );
    let editor = fixture.editor.clone();
    let width = |cx: &mut VisualTestContext| {
        body(&editor, cx)["content"][0]["attrs"]["editorWidth"].clone()
    };
    let id = editor.read_with(cx, |editor, _| {
        editor
            .model
            .as_ref()
            .unwrap()
            .document
            .root
            .children
            .get(0)
            .unwrap()
            .id
    });
    for (delta, expected) in [(100., 63.), (-1000., 15.), (2000., 100.)] {
        editor.update(cx, |editor, cx| {
            editor.resize = Some(super::pane::ImageResize {
                id,
                start: 0,
                left: false,
                origin: px(0.),
                initial: px(400.),
                max: px(800.),
                width: px(400.),
            });
            editor.resize_move(px(delta), cx);
            editor.resize_commit(cx);
        });
        assert_eq!(width(cx), json!(expected));
    }
}

struct ClosedIssues;

impl crate::workspace::task::Github for ClosedIssues {
    fn get(
        &self,
        _: String,
    ) -> futures::future::BoxFuture<'static, desktop_runtime::Result<Value>> {
        Box::pin(async { Ok(json!({"state": "closed"})) })
    }
}

#[gpui::test]
fn a_plain_click_on_a_github_chip_opens_its_task_tab_and_leaves_the_caret(cx: &mut TestAppContext) {
    cx.update(|cx| {
        cx.set_global(crate::workspace::task::GithubOverride(std::sync::Arc::new(
            ClosedIssues,
        )))
    });
    let chip = |kind: &str, number: u64| json!({"type": "appLink", "attrs": {"provider": "github", "kind": kind, "owner": "acme", "repo": "app", "number": number, "url": "https://github.com/acme/app"}});
    let (fixture, cx) = fixture(
        cx,
        vec![json!({"type": "paragraph", "content": [
            {"type": "text", "text": "Fix "}, chip("issue", 42), {"type": "text", "text": " and "}, chip("pull_request", 43)
        ]})],
    );
    let editor = fixture.editor.clone();
    let opened = Rc::new(RefCell::new(Vec::new()));
    cx.update(|_, cx| {
        let opened = opened.clone();
        cx.subscribe(&editor, move |_, request: &EditorRequest, _| {
            if let EditorRequest::OpenTask(route) = request {
                opened.borrow_mut().push(route.clone());
            }
        })
        .detach();
    });
    let laid_out = |cx: &mut VisualTestContext| {
        editor.read_with(cx, |editor, _| {
            editor.layouts.values().any(|layout| {
                layout
                    .row
                    .spans
                    .iter()
                    .any(|span| span.node.kind() == "appLink")
            })
        })
    };
    settle(cx, laid_out);
    let (target, caret) = editor.read_with(cx, |editor, _| {
        let layout = editor
            .layouts
            .values()
            .find(|layout| {
                layout
                    .row
                    .spans
                    .iter()
                    .any(|span| span.node.kind() == "appLink")
            })
            .expect("the paragraph is laid out");
        assert!(
            !layout.row.text.contains("appLink"),
            "the chip replaces the node's placeholder text"
        );
        let span = layout
            .row
            .spans
            .iter()
            .find(|span| span.node.kind() == "appLink")
            .unwrap();
        let start = layout.block_start + layout.row.start + span.position;
        let bounds = layout.bounds_for(start..start + span.units).unwrap();
        (bounds.center(), editor.model.as_ref().unwrap().selection)
    });
    cx.simulate_mouse_down(target, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
    assert_eq!(
        opened.borrow().as_slice(),
        [crate::workspace::navigation::Route::Task(
            "github_issue:acme/app#42 github_pr:acme/app#43".into()
        )]
    );
    assert_eq!(
        editor.read_with(cx, |editor, _| editor.model.as_ref().unwrap().selection),
        caret
    );
}
