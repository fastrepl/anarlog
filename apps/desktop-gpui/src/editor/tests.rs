use std::{sync::Arc, time::Instant};

use desktop_runtime::{CancellationToken, Profile, RuntimeHandle, SaveDocument, ServiceError};
use futures::{StreamExt, executor::block_on};
use serde_json::{Value, json};

use super::{
    clipboard,
    document::{Document, Text, utf8},
    menu::{MentionCandidate, MentionResults, MentionTarget},
    model::{EditorModel, Mapping, Selection},
    persistence::{SaveEvent, SaveJournal},
    surface::ProjectedBlock,
};

#[test]
fn document_boundary_navigation_reaches_nested_unicode_text_and_extends_selection() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"horizontalRule"},
        {"type":"paragraph","content":[{"type":"text","text":"first"}]},
        {"type":"bulletList","content":[{"type":"listItem","content":[
            {"type":"paragraph","content":[{"type":"text","text":"last 😀"}]}
        ]}]},
        {"type":"horizontalRule"}
    ]}));
    let start = caret_at(&editor, &[1], 0);
    let end = caret_at(&editor, &[2, 0, 0], 7);
    editor.select(Selection::caret(start + 2));
    editor.move_document_boundary(true, true);
    assert_eq!(
        editor.selection,
        Selection {
            anchor: start + 2,
            head: end
        }
    );
    assert_eq!(
        editor.document.resolve(end).unwrap().node.kind(),
        "paragraph"
    );
    editor.move_document_boundary(false, false);
    assert_eq!(editor.selection, Selection::caret(start));
    editor.move_document_boundary(true, false);
    editor.replace(editor.selection.range(), "!").unwrap();
    assert!(editor.document.serialize().unwrap().contains("last 😀!"));
    editor.undo().unwrap();
    assert_eq!(editor.selection, Selection::caret(end));
}

#[test]
fn virtual_projection_indexes_nested_lists_without_projecting_siblings() {
    let raw = json!({"type":"doc","content":[{"type":"orderedList","attrs":{"start":3},"content":
        (0..10_000).map(|index| json!({"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":format!("row {index} 😀")}]}]})).collect::<Vec<_>>()
    }]}).to_string();
    let mut model = EditorModel::new(Document::parse(raw.into()).unwrap());
    assert_eq!(model.document.root.children.render_blocks(), 10_000);
    let (first, _, _, _) = super::surface::block_target(&model.document, 0).unwrap();
    let (node, start, depth, marker) =
        super::surface::block_target(&model.document, 9_999).unwrap();
    assert_eq!(depth, 1);
    assert_eq!(marker, "10002.");
    let projected = ProjectedBlock::with_context(node.clone(), depth, marker);
    assert_eq!(projected.rows.len(), 1);
    assert_eq!(projected.rows[0].text.as_ref(), "row 9999 😀");
    assert_eq!(
        super::surface::block_index(&model.document, start + 1),
        Some(9_999)
    );
    model.select(Selection::caret(start + 1));
    model.replace(model.selection.range(), "Edited ").unwrap();
    assert!(Arc::ptr_eq(
        &first,
        &super::surface::block_target(&model.document, 0).unwrap().0
    ));
    assert!(!Arc::ptr_eq(
        &node,
        &super::surface::block_target(&model.document, 9_999)
            .unwrap()
            .0
    ));
    assert_eq!(model.document.root.children.render_blocks(), 10_000);
    model.undo().unwrap();
    assert!(Arc::ptr_eq(
        &node,
        &super::surface::block_target(&model.document, 9_999)
            .unwrap()
            .0
    ));
}

#[test]
fn vertical_navigation_crosses_virtual_blocks_at_safe_utf16_columns() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"abc"}]},
        {"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"😀xy"}]}]}]},
        {"type":"paragraph","content":[{"type":"text","text":"last"}]}
    ]}));
    let first = caret_at(&editor, &[0], 1);
    editor.select(Selection::caret(first));
    editor.move_vertical_blocks(true, 1, true).unwrap();
    assert_eq!(editor.selection.anchor, first);
    assert_eq!(editor.selection.head, caret_at(&editor, &[1, 0, 0], 0));
    editor.move_vertical_blocks(true, 20, false).unwrap();
    assert_eq!(
        editor.selection,
        Selection::caret(caret_at(&editor, &[2], 0))
    );
    editor.move_vertical_blocks(false, 20, false).unwrap();
    assert_eq!(
        editor.selection,
        Selection::caret(caret_at(&editor, &[0], 0))
    );
}

#[test]
fn hard_break_replaces_cross_container_selection_in_one_undo_step() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"ab"}]},
        {"type":"blockquote","attrs":{"future":"preserved"},"content":[{"type":"paragraph","content":[{"type":"text","text":"cdef"}]}]}
    ]}));
    let original = editor.document.root.value();
    editor.select(Selection {
        anchor: caret_at(&editor, &[0], 1),
        head: caret_at(&editor, &[1, 0], 2),
    });
    editor
        .insert_inline_atom("hardBreak", Default::default())
        .unwrap();
    assert!(editor.document.serialize().unwrap().contains("hardBreak"));
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
    editor.redo().unwrap();
    assert!(editor.document.serialize().unwrap().contains("hardBreak"));
}

#[test]
fn attachment_removal_checks_identity_and_preserves_undo_and_valid_cells() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"table","content":[{"type":"tableRow","content":[{"type":"tableCell","content":[
            {"type":"image","attrs":{"attachmentId":"image.png","future":"retained"}}
        ]}]}]}
    ]}));
    let path = [0, 0, 0, 0];
    let position = editor.document.position_at_path(&path).unwrap();
    let id = editor.document.node(&path).unwrap().id;
    let original = editor.document.root.value();
    assert!(editor.remove_block_atom(position, id + 1).is_err());
    assert_eq!(editor.document.root.value(), original);
    editor.remove_block_atom(position, id).unwrap();
    assert_eq!(editor.document.node(&path).unwrap().kind(), "paragraph");
    editor.document.resolve(editor.selection.head).unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn code_conversion_removes_known_marks_and_preserves_future_marks_on_failure() {
    let mut editor = fixture(
        json!({"type":"doc","content":[{"type":"paragraph","content":[
            {"type":"text","text":"bold","marks":[{"type":"bold"}]},
            {"type":"hardBreak"},{"type":"text","text":"line"}
        ]}]}),
    );
    let original = editor.document.root.value();
    editor.set_block("codeBlock", None).unwrap();
    Document::parse(editor.document.serialize().unwrap()).unwrap();
    assert_eq!(
        super::document::inline_text(&editor.document.node(&[0]).unwrap().children),
        "bold\nline"
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
    let mut editor = fixture(
        json!({"type":"doc","content":[{"type":"paragraph","content":[
            {"type":"text","text":"future","marks":[{"type":"future","attrs":{"keep":true}}]}
        ]}]}),
    );
    let original = editor.document.root.value();
    assert!(editor.set_block("codeBlock", None).is_err());
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn future_inline_nodes_block_destructive_replacement_without_blocking_adjacent_edits() {
    let mut editor = fixture(
        json!({"type":"doc","content":[{"type":"paragraph","content":[
        {"type":"text","text":"a"},{"type":"future-inline","attrs":{"secret":"preserve"}},
        {"type":"text","text":"b"}
    ]},{"type":"paragraph","content":[{"type":"text","text":"next"}]}]}),
    );
    let original = editor.document.root.value();
    let start = caret_at(&editor, &[0], 1);
    let end = caret_at(&editor, &[0], 2);
    assert!(editor.replace(start..end, "").is_err());
    let across = caret_at(&editor, &[1], 1);
    assert!(editor.replace(start..across, "").is_err());
    editor.select(Selection {
        anchor: start,
        head: end,
    });
    assert!(
        editor
            .insert_inline_atom("hardBreak", Default::default())
            .is_err()
    );
    let fragment = Document::parse(Arc::from(r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"paste"}]}]}"#)).unwrap();
    assert!(editor.insert_slice(fragment, true).is_err());
    assert_eq!(editor.document.root.value(), original);
    editor.replace(start..start, "safe").unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn rich_paste_into_code_uses_plain_lines_in_one_undo_step() {
    let mut editor = fixture(
        json!({"type":"doc","content":[{"type":"codeBlock","content":[{"type":"text","text":"code"}]}]}),
    );
    let original = editor.document.root.value();
    let (fragment, _) = super::html::parse("<p><strong>one</strong></p><p>two</p>").unwrap();
    editor.insert_slice(fragment, false).unwrap();
    assert_eq!(
        super::document::inline_text(&editor.document.node(&[0]).unwrap().children),
        "one\ntwocode"
    );
    Document::parse(editor.document.serialize().unwrap()).unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn journal_flush_roundtrips_rich_document_and_retains_conflicting_draft() {
    let directory = tempfile::tempdir().unwrap();
    let profile = Profile {
        database: directory.path().join("library.sqlite"),
    };
    let (runtime, ready) = RuntimeHandle::start(profile.clone()).unwrap();
    block_on(ready.receive()).unwrap();
    let session = block_on(
        runtime
            .create_note("Rich fixture".into())
            .unwrap()
            .receive(),
    )
    .unwrap();
    let base = session.note.unwrap();
    let rich = json!({
        "type": "doc", "attrs": {"future": 42}, "content": [
            {"type": "paragraph", "content": [{"type":"text","text":"日本語 😀", "marks":[{"type":"bold"}]}]},
            {"type": "future-widget", "attrs": {"attachmentId":"opaque-id", "payload":[1,2,3]}},
            {"type": "table", "content": [{"type":"tableRow", "content":[{"type":"tableCell", "attrs":{"colspan":1,"rowspan":1}, "content":[{"type":"paragraph","content":[{"type":"text","text":"Cell"}]}]}]}]},
            {"type":"taskList","content":[{"type":"taskItem","attrs":{"checked":true},"content":[{"type":"paragraph","content":[{"type":"text","text":"Done"}]}]}]}
        ]
    });
    let mut editor = EditorModel::new(Document::parse(rich.to_string().into()).unwrap());
    editor.replace(1..1, "Edited ").unwrap();
    let expected = editor.document.serialize_for_save().unwrap();
    let (journal, mut events) = SaveJournal::start(runtime.clone(), base).unwrap();
    journal.publish(editor.revision, editor.document.clone());
    block_on(journal.flush(editor.revision)).unwrap().unwrap();
    let saved = match block_on(events.next()).unwrap() {
        SaveEvent::Saved { snapshot, .. } => snapshot,
        SaveEvent::Failed(error) => panic!("{error}"),
    };
    assert_eq!(saved.body, expected);
    let remote: Arc<str> = r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"Remote"}]}]}"#.into();
    block_on(
        runtime
            .save_document(SaveDocument {
                base: saved,
                body: remote.clone(),
            })
            .unwrap()
            .receive(),
    )
    .unwrap();
    editor.replace(1..1, "Local ").unwrap();
    journal.publish(editor.revision, editor.document.clone());
    assert!(matches!(
        block_on(journal.flush(editor.revision)).unwrap(),
        Err(ServiceError::Conflict)
    ));
    assert!(matches!(
        block_on(events.next()).unwrap(),
        SaveEvent::Failed(ServiceError::Conflict)
    ));
    assert!(
        editor
            .document
            .serialize()
            .unwrap()
            .contains("Local Edited")
    );
    let current = block_on(
        runtime
            .open_session(session.summary.id.clone(), CancellationToken::new())
            .unwrap()
            .receive(),
    )
    .unwrap()
    .note
    .unwrap();
    assert_eq!(current.body, remote);
    journal
        .resolve_conflict(current, editor.revision, editor.document.clone())
        .unwrap();
    block_on(journal.flush(editor.revision)).unwrap().unwrap();
    assert!(matches!(
        block_on(events.next()).unwrap(),
        SaveEvent::Saved { .. }
    ));
    let expected = editor.document.serialize_for_save().unwrap();
    drop(journal);
    drop(events);
    block_on(runtime.shutdown()).unwrap();
    let (runtime, ready) = RuntimeHandle::start(profile).unwrap();
    block_on(ready.receive()).unwrap();
    let restored = block_on(
        runtime
            .open_session(session.summary.id, CancellationToken::new())
            .unwrap()
            .receive(),
    )
    .unwrap()
    .note
    .unwrap();
    assert_eq!(restored.body, expected);
    let restored: Value = serde_json::from_str(&restored.body).unwrap();
    assert_eq!(restored["content"][1], rich["content"][1]);
    assert_eq!(restored["content"][2], rich["content"][2]);
    assert_eq!(restored["content"][3], rich["content"][3]);
    block_on(runtime.shutdown()).unwrap();
}

fn model(text: &str) -> EditorModel {
    EditorModel::new(Document::parse(serde_json::json!({
        "type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": text}]}]
    }).to_string().into()).unwrap())
}

fn fixture(value: Value) -> EditorModel {
    EditorModel::new(Document::parse(value.to_string().into()).unwrap())
}

fn caret_at(editor: &EditorModel, path: &[usize], offset: usize) -> usize {
    editor.document.position_at_path(path).unwrap() + 1 + offset
}

#[test]
fn replacement_across_quote_and_nested_list_preserves_siblings_and_history() {
    let paragraph = |text| json!({"type":"paragraph","content":[{"type":"text","text":text}]});
    let original = json!({"type":"doc","attrs":{"future":true},"content":[
        {"type":"blockquote","attrs":{"future":"quote"},"content":[paragraph("alpha"),paragraph("remove")]},
        {"type":"bulletList","content":[
            {"type":"listItem","content":[paragraph("beta")]},
            {"type":"listItem","content":[paragraph("keep")]}
        ]}, paragraph("outside")
    ]});
    let mut editor = fixture(original.clone());
    let shared = editor.document.root.children.get(2).unwrap().clone();
    let from = caret_at(&editor, &[0, 0], 2);
    let to = caret_at(&editor, &[1, 0, 0], 2);
    editor.replace(from..to, "😀").unwrap();
    let changed = editor.document.root.value();
    assert_eq!(
        changed["content"][0]["content"][0]["content"][0]["text"],
        "al😀ta"
    );
    assert_eq!(
        changed["content"][1]["content"][0]["content"][0],
        paragraph("keep")
    );
    assert!(Arc::ptr_eq(
        &shared,
        editor.document.root.children.get(2).unwrap()
    ));
    editor.replace(editor.selection.range(), "!").unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), changed);
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
    editor.redo().unwrap();
    assert_eq!(editor.document.root.value(), changed);
}

#[test]
fn opaque_intersection_rolls_back_selected_split_and_history() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"before"}]},
        {"type":"future-widget","attrs":{"payload":{"must":"survive"}}},
        {"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"after"}]}]}
    ]}));
    let original = editor.document.root.clone();
    editor.select(Selection {
        anchor: 3,
        head: caret_at(&editor, &[2, 0], 2),
    });
    let selection = editor.selection;
    assert!(editor.split_block().is_err());
    assert!(Arc::ptr_eq(&original, &editor.document.root));
    assert_eq!(editor.selection, selection);
    assert_eq!(editor.revision, 0);
    editor.undo().unwrap();
    assert!(Arc::ptr_eq(&original, &editor.document.root));
}

#[test]
fn whole_document_rich_paste_retains_unknown_nodes_on_failure() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"known"}]},
        {"type":"future-widget","attrs":{"payload":42}}
    ]}));
    let original = editor.document.root.clone();
    editor.select(Selection {
        anchor: 0,
        head: editor.document.units(),
    });
    let selection = editor.selection;
    let (fragment, open) = super::html::parse("<p>replacement</p>").unwrap();
    assert!(editor.insert_slice(fragment, open).is_err());
    assert!(Arc::ptr_eq(&original, &editor.document.root));
    assert_eq!(editor.selection, selection);
    assert_eq!(editor.revision, 0);
}

#[test]
fn selected_attachment_insertion_in_list_and_cell_is_one_undo_step() {
    let paragraph =
        json!({"type":"paragraph","content":[{"type":"text","text":"before selected after"}]});
    for container in [
        json!({"type":"bulletList","content":[{"type":"listItem","content":[paragraph.clone()]}]}),
        json!({"type":"table","content":[{"type":"tableRow","content":[{"type":"tableCell","content":[paragraph]}]}]}),
    ] {
        let mut editor = fixture(json!({"type":"doc","content":[container]}));
        let original = editor.document.root.clone();
        let start = editor.document.first_caret() + 7;
        editor.select(Selection {
            anchor: start,
            head: start + 8,
        });
        editor
            .insert_block_atom(
                "fileAttachment",
                json!({"attachmentId":"catalogued-id","name":"report.pdf"})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
            .unwrap();
        Document::parse(editor.document.root.value().to_string().into()).unwrap();
        assert!(
            editor
                .document
                .root
                .value()
                .to_string()
                .contains("catalogued-id")
        );
        editor.undo().unwrap();
        assert!(Arc::ptr_eq(&original, &editor.document.root));
    }
}

#[test]
fn table_tab_skips_rows_covered_by_vertical_spans() {
    let cell = |text, rowspan| json!({"type":"tableCell","attrs":{"rowspan":rowspan},"content":[{"type":"paragraph","content":[{"type":"text","text":text}]}]});
    let mut editor = fixture(json!({"type":"doc","content":[{"type":"table","content":[
        {"type":"tableRow","content":[cell("top",2)]},
        {"type":"tableRow","content":[]},
        {"type":"tableRow","content":[cell("bottom",1)]}
    ]}]}));
    let first = caret_at(&editor, &[0, 0, 0, 0], 0);
    let last = caret_at(&editor, &[0, 2, 0, 0], 0);
    editor.select(Selection::caret(first));
    editor.table_move(true).unwrap();
    assert_eq!(editor.selection.head, last);
    editor.table_move(false).unwrap();
    assert_eq!(editor.selection.head, first);
    editor.select(Selection::caret(last));
    let original = editor.document.root.clone();
    editor.table_move(true).unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    editor.undo().unwrap();
    assert!(Arc::ptr_eq(&original, &editor.document.root));
}

#[test]
fn selected_split_and_nested_rich_paste_are_single_undo_steps() {
    let mut editor = fixture(
        json!({"type":"doc","content":[{"type":"bulletList","content":[
            {"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"abcdef"}]}]}
        ]}]}),
    );
    let original = editor.document.root.value();
    let start = caret_at(&editor, &[0, 0, 0], 2);
    editor.select(Selection {
        anchor: start,
        head: start + 2,
    });
    editor.split_block().unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
    editor.select(Selection::caret(start));
    let (slice, open) =
        super::html::parse("<p><b>X</b></p><ul><li><p>nested</p></li></ul>").unwrap();
    editor.insert_slice(slice, open).unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["content"][2]["type"],
        "bulletList"
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn nonterminal_list_exit_retains_trailing_items_and_task_split_allocates_identity() {
    let empty = json!({"type":"paragraph","content":[]});
    let list_item = |paragraph| json!({"type":"listItem","content":[paragraph]});
    let mut editor = fixture(
        json!({"type":"doc","content":[{"type":"bulletList","content":[
            list_item(empty.clone()), list_item(json!({"type":"paragraph","content":[{"type":"text","text":"later"}]}))
        ]}]}),
    );
    editor.split_block().unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["type"],
        "paragraph"
    );
    assert_eq!(
        editor.document.root.value()["content"][1]["type"],
        "bulletList"
    );
    assert!(editor.document.resolve(editor.selection.head).is_ok());
    let mut task = fixture(
        json!({"type":"doc","content":[{"type":"taskList","content":[
            {"type":"taskItem","attrs":{"taskId":"task","taskItemId":"item","status":"done","checked":true},
             "content":[{"type":"paragraph","content":[{"type":"text","text":"task"}]}]}
        ]}]}),
    );
    task.select(Selection::caret(caret_at(&task, &[0, 0, 0], 2)));
    task.split_block().unwrap();
    let value = task.document.root.value();
    assert_eq!(value["content"][0]["content"][0]["attrs"]["taskId"], "task");
    let attrs = &value["content"][0]["content"][1]["attrs"];
    assert_ne!(attrs["taskId"], "task");
    assert_ne!(attrs["taskItemId"], "item");
    assert_eq!(attrs["checked"], false);
    assert_eq!(attrs["status"], "todo");
}

#[test]
fn html_clipboard_entities_nested_lists_grid_and_unsafe_schemes() {
    let (document, _) = super::html::parse(
        "<h2>A &amp; B</h2><ul><li><p><strong>one</strong></p><ol start='4'><li>two</li></ol></li></ul>\
         <table><tr><th colspan='2'>head</th></tr><tr><td>a<br>b</td><td><a href='https://example.com'>link</a></td></tr></table>"
    ).unwrap();
    let value = document.root.value();
    assert_eq!(value["content"][0]["content"][0]["text"], "A & B");
    assert_eq!(
        value["content"][1]["content"][0]["content"][1]["attrs"]["start"],
        4
    );
    let grid = ProjectedBlock::build(document.root.children.get(2).unwrap().clone())
        .grid
        .unwrap();
    assert_eq!(grid.columns, 2);
    assert_eq!(grid.cells.len(), 3);
    assert_eq!(grid.cells[0].colspan, 2);
    assert!(grid.cells[0].header);
    assert!(super::html::parse("<p><a href='javascript:alert(1)'>x</a></p>").is_err());
}

#[test]
fn grid_edit_preserves_structure_and_tab_adds_undoable_row() {
    let p = |text| json!({"type":"paragraph","content":[{"type":"text","text":text}]});
    let cell = |text| json!({"type":"tableCell","attrs":{"future":true},"content":[p(text)]});
    let mut editor = fixture(json!({"type":"doc","content":[{"type":"table","content":[
        {"type":"tableRow","content":[cell("abc"), cell("def")]}
    ]}]}));
    let original = editor.document.root.value();
    let start = caret_at(&editor, &[0, 0, 0, 0], 1);
    let end = caret_at(&editor, &[0, 0, 1, 0], 2);
    editor.replace(start..end, "X").unwrap();
    let value = editor.document.root.value();
    assert_eq!(
        value["content"][0]["content"][0]["content"][0]["content"][0],
        p("aX")
    );
    assert_eq!(
        value["content"][0]["content"][0]["content"][1]["content"][0],
        p("f")
    );
    assert_eq!(
        value["content"][0]["content"][0]["content"][1]["attrs"]["future"],
        true
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
    editor.select(Selection::caret(caret_at(&editor, &[0, 0, 1, 0], 1)));
    editor.table_move(true).unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(editor.document.resolve(editor.selection.head).is_ok());
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn multiline_composition_across_containers_can_cancel_and_commit() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"before"}]},
        {"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"after"}]}]}
    ]}));
    let original = editor.document.root.value();
    let range = 3..caret_at(&editor, &[1, 0], 2);
    editor.compose(Some(range.clone()), "日\n😀", 4..4).unwrap();
    editor.compose(None, "日本\n😀", 5..5).unwrap();
    editor.cancel_composition();
    assert_eq!(editor.document.root.value(), original);
    editor.compose(Some(range), "日\n😀", 4..4).unwrap();
    editor.commit_composition();
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn runtime_mentions_read_all_three_domains_and_cancel() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("library.sqlite"),
    })
    .unwrap();
    block_on(ready.receive()).unwrap();
    block_on(runtime.create_note("Zed Session".into()).unwrap().receive()).unwrap();
    block_on(
        runtime
            .read(CancellationToken::new(), |services| async move {
                services
                    .executor
                    .execute(
                        "INSERT INTO humans (id,name) VALUES ('human','Zed Human')".into(),
                        vec![],
                    )
                    .await
                    .unwrap();
                services
                    .executor
                    .execute(
                        "INSERT INTO organizations (id,name) VALUES ('org','Zed Org')".into(),
                        vec![],
                    )
                    .await
                    .unwrap();
                Ok(())
            })
            .unwrap()
            .receive(),
    )
    .unwrap();
    let results = block_on(super::services::mentions(
        runtime.clone(),
        "Zed".into(),
        CancellationToken::new(),
    ))
    .unwrap();
    assert_eq!(results.len(), 3);
    assert!(
        results
            .iter()
            .any(|r| matches!(r.target, MentionTarget::Human(_)))
    );
    assert!(
        results
            .iter()
            .any(|r| matches!(r.target, MentionTarget::Session(_)))
    );
    assert!(
        results
            .iter()
            .any(|r| matches!(r.target, MentionTarget::Organization(_)))
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(
        block_on(super::services::mentions(
            runtime.clone(),
            "Zed".into(),
            cancel
        ))
        .is_err()
    );
    block_on(runtime.shutdown()).unwrap();
}

#[test]
fn document_watch_delivers_revisions_and_unsubscribes_under_backpressure() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("library.sqlite"),
    })
    .unwrap();
    block_on(ready.receive()).unwrap();
    let session = block_on(runtime.create_note("Watch".into()).unwrap().receive()).unwrap();
    let cancel = CancellationToken::new();
    let worker_cancel = cancel.clone();
    let worker_runtime = runtime.clone();
    let (sender, mut receiver) = futures::channel::mpsc::channel(0);
    let watcher = std::thread::spawn(move || {
        block_on(super::services::watch(
            worker_runtime,
            session.summary.id,
            worker_cancel,
            sender,
        ));
    });
    let initial = block_on(receiver.next()).unwrap().unwrap().unwrap();
    assert_eq!(initial.body, session.note.unwrap().body);
    let body = model("remote update").document.serialize().unwrap();
    let saved = block_on(
        runtime
            .save_document(desktop_runtime::SaveDocument {
                base: initial,
                body: body.clone(),
            })
            .unwrap()
            .receive(),
    )
    .unwrap();
    let changed = block_on(receiver.next()).unwrap().unwrap().unwrap();
    assert_eq!(changed.body, saved.body);
    assert_eq!(changed.updated_at, saved.updated_at);
    assert_eq!(changed.id, saved.id);
    block_on(
        runtime
            .save_document(desktop_runtime::SaveDocument {
                base: saved,
                body: model("pending delivery").document.serialize().unwrap(),
            })
            .unwrap()
            .receive(),
    )
    .unwrap();
    cancel.cancel();
    watcher.join().unwrap();
    drop(receiver);
    block_on(runtime.shutdown()).unwrap();
}

#[test]
fn attachment_import_catalogues_local_state_and_rolls_back_deleted_session() {
    let directory = tempfile::tempdir().unwrap();
    let (runtime, ready) = RuntimeHandle::start(Profile {
        database: directory.path().join("library.sqlite"),
    })
    .unwrap();
    block_on(ready.receive()).unwrap();
    let session = block_on(runtime.create_note("Attachments".into()).unwrap().receive()).unwrap();
    let source = directory.path().join("source.txt");
    std::fs::write(&source, "attachment bytes").unwrap();
    let vault = directory.path().join("vault");
    let service = super::AttachmentService::new(runtime.clone(), vault.clone());
    let imported = block_on(service.import(session.summary.id.clone(), source.clone())).unwrap();
    assert_eq!(
        std::fs::read_to_string(imported.path.as_ref()).unwrap(),
        "attachment bytes"
    );
    assert_eq!(imported.mime, "text/plain");
    let resolved = block_on(service.resolve(
        session.summary.id.clone(),
        imported.id.clone(),
        CancellationToken::new(),
    ))
    .unwrap();
    assert_eq!(resolved.size, 16);
    let session_id = session.summary.id.clone();
    let rows = block_on(runtime.read(CancellationToken::new(), move |services| async move {
        Ok(services.executor.execute("SELECT a.id,a.storage_kind,l.availability,a.sha256,a.cloud_sync_enabled FROM session_attachments a JOIN attachment_local_state l ON l.attachment_id=a.id WHERE a.session_id=?".into(), vec![json!(session_id)]).await.unwrap())
    }).unwrap().receive()).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["availability"], "present");
    assert_eq!(rows[0]["storage_kind"], "local_file");
    assert_eq!(rows[0]["sha256"].as_str().unwrap().len(), 64);
    let shared = block_on(service.resolve(
        session.summary.id.clone(),
        rows[0]["id"].as_str().unwrap().into(),
        CancellationToken::new(),
    ))
    .unwrap();
    assert_eq!(shared.path, resolved.path);
    assert!(
        block_on(service.resolve(
            session.summary.id.clone(),
            "../source.txt".into(),
            CancellationToken::new()
        ))
        .is_err()
    );
    let other = block_on(runtime.create_note("Other".into()).unwrap().receive()).unwrap();
    assert!(
        block_on(service.resolve(other.summary.id, imported.id, CancellationToken::new())).is_err()
    );
    let session_id = session.summary.id.clone();
    block_on(
        runtime
            .submit(move |services| async move {
                services
                    .executor
                    .execute(
                        "UPDATE sessions SET deleted_at='2026-01-01' WHERE id=?".into(),
                        vec![json!(session_id)],
                    )
                    .await
                    .unwrap();
                Ok(())
            })
            .unwrap()
            .receive(),
    )
    .unwrap();
    assert!(block_on(service.import(session.summary.id.clone(), source)).is_err());
    assert_eq!(
        anlg_fs_sync_core::FsSyncCore::new(vault)
            .attachment_list(&session.summary.id.0)
            .unwrap()
            .len(),
        1
    );
    block_on(runtime.shutdown()).unwrap();
}

#[test]
fn marks_apply_uniformly_across_containers_without_discarding_future_marks() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"marked","marks":[{"type":"bold"},{"type":"future","attrs":{"id":3}}]}]},
        {"type":"blockquote","content":[{"type":"paragraph","content":[{"type":"text","text":"plain"}]}]}
    ]}));
    let original = editor.document.root.value();
    editor.select(Selection {
        anchor: 1,
        head: caret_at(&editor, &[1, 0], 5),
    });
    editor.toggle_mark("bold").unwrap();
    for path in [&[0][..], &[1, 0][..]] {
        assert!(
            editor
                .document
                .node(path)
                .unwrap()
                .children
                .get(0)
                .unwrap()
                .marks()
                .iter()
                .any(|m| m["type"] == "bold")
        );
    }
    editor.set_link("https://example.com").unwrap();
    editor.undo().unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
    assert!(editor.toggle_mark("code").is_err());
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn arrows_cross_nested_container_boundaries_in_both_directions() {
    let mut editor = fixture(json!({"type":"doc","content":[
        {"type":"paragraph","content":[{"type":"text","text":"one"}]},
        {"type":"bulletList","content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"two"}]}]}]},
        {"type":"paragraph","content":[{"type":"text","text":"three"}]}
    ]}));
    let first_end = caret_at(&editor, &[0], 3);
    let nested_start = caret_at(&editor, &[1, 0, 0], 0);
    let nested_end = caret_at(&editor, &[1, 0, 0], 3);
    let last_start = caret_at(&editor, &[2], 0);
    editor.select(Selection::caret(first_end));
    editor.move_grapheme(true, false).unwrap();
    assert_eq!(editor.selection.head, nested_start);
    editor.move_grapheme(false, false).unwrap();
    assert_eq!(editor.selection.head, first_end);
    editor.select(Selection::caret(nested_end));
    editor.move_word(true, false).unwrap();
    assert_eq!(editor.selection.head, last_start);
    editor.move_word(false, false).unwrap();
    assert_eq!(editor.selection.head, nested_end);
}

#[test]
fn row_insertion_extends_vertical_spans_and_preserves_cell_attributes() {
    let mut editor = fixture(json!({"type":"doc","content":[{"type":"table","content":[
        {"type":"tableRow","content":[
            {"type":"tableCell","attrs":{"rowspan":2,"future":"retained"},"content":[{"type":"paragraph","content":[{"type":"text","text":"span"}]}]},
            {"type":"tableCell","content":[{"type":"paragraph","content":[{"type":"text","text":"first"}]}]}
        ]},
        {"type":"tableRow","content":[{"type":"tableCell","content":[{"type":"paragraph","content":[{"type":"text","text":"second"}]}]}]}
    ]}]}));
    let original = editor.document.root.value();
    editor.select(Selection::caret(caret_at(&editor, &[0, 0, 1, 0], 1)));
    editor.table_add_row().unwrap();
    let value = editor.document.root.value();
    assert_eq!(
        value["content"][0]["content"][0]["content"][0]["attrs"]["rowspan"],
        3
    );
    assert_eq!(
        value["content"][0]["content"][0]["content"][0]["attrs"]["future"],
        "retained"
    );
    assert_eq!(
        value["content"][0]["content"][1]["content"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        value["content"][0]["content"][2],
        original["content"][0]["content"][1]
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), original);
}

#[test]
fn composition_is_one_undo_and_cancel_restores_selection() {
    let mut editor = model("hello");
    editor.select(Selection::caret(6));
    editor.compose(None, "に", 1..1).unwrap();
    editor.compose(None, "日本", 0..2).unwrap();
    assert_eq!(editor.selection, Selection { anchor: 6, head: 8 });
    editor.commit_composition();
    editor.undo().unwrap();
    assert_eq!(
        editor.document.serialize().unwrap().as_ref(),
        model("hello").document.serialize().unwrap().as_ref()
    );
    editor.redo().unwrap();
    assert!(editor.document.serialize().unwrap().contains("hello日本"));
    editor.compose(None, "語", 1..1).unwrap();
    editor.cancel_composition();
    assert!(editor.document.serialize().unwrap().contains("hello日本"));
}

#[test]
fn schema_roundtrip_preserves_unknown_data_after_neighbor_edit() {
    let paragraph = json!({"type":"paragraph","attrs":{"future":9},"content":[
        {"type":"text","text":"edit","vendor":{"x":[1,2]},"marks":[
            {"type":"bold"},{"type":"italic"},{"type":"underline"},{"type":"strike"},
            {"type":"highlight"},{"type":"future-mark","attrs":{"opaque":true}}
        ]},
        {"type":"hardBreak"},{"type":"mention-@","attrs":{"id":"person","type":"human","label":"名字"}},
        {"type":"appLink","attrs":{"provider":"github","resourceId":"123","label":"issue"}}
    ]});
    let node_types = [
        "horizontalRule",
        "image",
        "fileAttachment",
        "session",
        "clip",
    ];
    let mut blocks = vec![paragraph.clone()];
    for kind in node_types {
        blocks
            .push(json!({"type":kind,"attrs":{"attachmentId":"portable","future": {"keep":true}}}));
    }
    for level in 1..=6 {
        blocks.push(json!({"type":"heading","attrs":{"level":level},"content":[{"type":"text","text":"Title"}]}));
    }
    blocks.push(json!({"type":"codeBlock","content":[{"type":"text","text":"code\n日本"}]}));
    blocks.push(json!({"type":"blockquote","content":[paragraph.clone()]}));
    blocks.push(json!({"type":"orderedList","attrs":{"start":7},"content":[{"type":"listItem","content":[paragraph.clone(),{"type":"bulletList","content":[{"type":"listItem","content":[paragraph.clone()]}]}]}]}));
    blocks.push(json!({"type":"taskList","content":[{"type":"taskItem","attrs":{"taskId":"task","taskItemId":"item","status":"pending","checked":false},"content":[paragraph.clone()]}]}));
    blocks.push(json!({"type":"table","content":[{"type":"tableRow","content":[{"type":"tableHeader","attrs":{"colspan":2,"rowspan":1,"colwidth":[100,140]},"content":[paragraph.clone()]},{"type":"tableCell","attrs":{"colspan":1,"rowspan":2,"colwidth":null},"content":[paragraph]}]}]}));
    blocks.push(json!({"type":"opaque-future","attrs":{"keep":"exact"},"content":[{"type":"text","text":"hidden extension"}]}));
    let original = json!({"type":"doc","futureRoot":[false,42],"content":blocks});
    let bytes: Arc<str> = format!(" \n{}\n", original).into();
    let document = Document::parse(bytes.clone()).unwrap();
    assert!(Arc::ptr_eq(&bytes, &document.serialize().unwrap()));
    let mut editor = EditorModel::new(document);
    editor.replace(2..2, "X").unwrap();
    let changed: Value = serde_json::from_str(&editor.document.serialize().unwrap()).unwrap();
    assert_eq!(changed["futureRoot"], original["futureRoot"]);
    assert_eq!(
        changed["content"][0]["attrs"],
        original["content"][0]["attrs"]
    );
    assert_eq!(
        changed["content"][0]["content"][0]["vendor"],
        original["content"][0]["content"][0]["vendor"]
    );
    assert_eq!(
        changed["content"][0]["content"][0]["marks"],
        original["content"][0]["content"][0]["marks"]
    );
    for index in 1..original["content"].as_array().unwrap().len() {
        assert_eq!(changed["content"][index], original["content"][index]);
    }
}

#[test]
fn malformed_document_never_becomes_an_editable_blank() {
    for body in [
        "oops",
        "null",
        "{}",
        r#"{"type":"doc","content":[]}"#,
        r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"heading"}]}]}"#,
        r#"{"type":"doc","content":[{"type":"paragraph","marks":false}]}"#,
    ] {
        assert!(Document::parse(body.into()).is_err(), "{body}");
    }
}

#[test]
fn utf16_boundaries_and_large_text_pieces() {
    assert_eq!(utf8("a😀文", 3).unwrap(), 5);
    assert!(utf8("a😀文", 2).is_err());
    let text = Text::new(&"😀é漢".repeat(20_000));
    let original = text.as_string();
    let (a, b) = text.split(40_002).unwrap();
    assert_eq!(a.concat(&b).as_string(), original);
    assert!(text.split(40_001).is_err());
}

#[test]
fn grapheme_navigation_handles_combining_emoji_rtl_and_cjk() {
    for cluster in ["e\u{301}", "👨‍👩‍👧‍👦", "🇯🇵", "😀", "ש\u{5b8}", "漢"]
    {
        let mut editor = model(&format!("a{cluster}b"));
        editor.select(Selection::caret(2));
        editor.move_grapheme(true, false).unwrap();
        assert_eq!(
            editor.selection.head,
            2 + cluster.encode_utf16().count(),
            "{cluster}"
        );
        editor.delete(false).unwrap();
        assert!(editor.document.serialize().unwrap().contains("\"ab\""));
        editor.undo().unwrap();
        assert!(editor.document.serialize().unwrap().contains(cluster));
    }
}

#[test]
fn reversed_multiblock_selection_and_undo_preserve_fields() {
    let doc = json!({"type":"doc","content":[{"type":"paragraph","attrs":{"x":1},"content":[{"type":"text","text":"abc"}]},{"type":"paragraph","attrs":{"x":2},"content":[{"type":"text","text":"def"}]}]});
    let mut editor = EditorModel::new(Document::parse(doc.to_string().into()).unwrap());
    editor.select(Selection { anchor: 8, head: 2 });
    assert_eq!(
        clipboard::copy(&editor.document, editor.selection)
            .unwrap()
            .text,
        "bc\n\nde"
    );
    editor.replace(editor.selection.range(), "X").unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["text"],
        "aXf"
    );
    editor.undo().unwrap();
    assert_eq!(editor.document.root.value(), doc);
    assert_eq!(editor.selection, Selection { anchor: 8, head: 2 });
}

#[test]
fn link_is_noninclusive_but_unknown_marks_survive_typing() {
    let doc = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"web","marks":[{"type":"link","attrs":{"href":"https://example.com","target":"_blank"}},{"type":"future"}]}]}]});
    let mut editor = EditorModel::new(Document::parse(doc.to_string().into()).unwrap());
    editor.replace(4..4, "!").unwrap();
    let value = editor.document.root.value();
    assert_eq!(
        value["content"][0]["content"][1]["marks"],
        json!([{"type":"future"}])
    );
    assert!(clipboard::openable_link("HTTPS://example.com"));
    for link in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "https://",
        "http://a\nb",
    ] {
        assert!(!clipboard::openable_link(link));
    }
}

#[test]
fn rich_clipboard_preserves_marks_and_portable_attachment_metadata() {
    let original = json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"bold","marks":[{"type":"bold"}]}]},{"type":"image","attrs":{"attachmentId":"img-1","alt":"[a](b)","editorWidth":80}}]});
    let document = Document::parse(original.to_string().into()).unwrap();
    let payload = clipboard::copy(
        &document,
        Selection {
            anchor: 0,
            head: document.units(),
        },
    )
    .unwrap();
    assert!(payload.text.contains("![\\[a\\]\\(b\\)]()"));
    let (fragment, open) = clipboard::parse_slice(&payload.metadata).unwrap();
    assert!(!open);
    let mut editor = model("replace");
    editor.select(Selection {
        anchor: 0,
        head: editor.document.units(),
    });
    editor.insert_slice(fragment, open).unwrap();
    assert_eq!(editor.document.root.value(), original);
    assert!(!editor.document.serialize().unwrap().contains("file://"));
}

#[test]
fn whole_document_cut_paste_preserves_structure_and_undo() {
    for content in [
        json!([{"type":"paragraph","content":[{"type":"text","text":"ABCBC"}]}]),
        json!([
            {"type":"heading","attrs":{"level":2},"content":[{"type":"text","text":"日本語 😀"}]},
            {"type":"paragraph","content":[]},
            {"type":"paragraph","content":[{"type":"text","text":"bold","marks":[{"type":"bold"}]}]},
            {"type":"bulletList","content":[{"type":"listItem","content":[
                {"type":"paragraph","content":[{"type":"text","text":"nested"}]}
            ]}]}
        ]),
    ] {
        let mut editor = fixture(json!({"type":"doc","content":content}));
        let original = editor.document.root.value();
        for _ in 0..3 {
            editor.select(Selection {
                anchor: 0,
                head: editor.document.units(),
            });
            let payload = clipboard::copy(&editor.document, editor.selection).unwrap();
            editor.replace(editor.selection.range(), "").unwrap();
            let empty = editor.document.root.value();
            let (fragment, open) = clipboard::parse_slice(&payload.metadata).unwrap();
            editor.insert_slice(fragment, open).unwrap();
            assert_eq!(editor.document.root.value(), original);
            assert_eq!(
                editor.selection,
                Selection::caret(editor.document.edge_caret(true))
            );
            editor.undo().unwrap();
            assert_eq!(editor.document.root.value(), empty);
            editor.undo().unwrap();
            assert_eq!(editor.document.root.value(), original);
            editor.redo().unwrap();
            assert_eq!(editor.document.root.value(), empty);
            editor.redo().unwrap();
            assert_eq!(editor.document.root.value(), original);
        }
    }
}

#[test]
fn rich_inline_paste_inherits_slice_marks_without_losing_surroundings() {
    let mut source = model("bold");
    source.select(Selection { anchor: 1, head: 5 });
    source.toggle_mark("bold").unwrap();
    let payload = clipboard::copy(&source.document, source.selection).unwrap();
    let mut destination = model("ab");
    destination.select(Selection::caret(2));
    let (fragment, open) = clipboard::parse_slice(&payload.metadata).unwrap();
    destination.insert_slice(fragment, open).unwrap();
    let value = destination.document.root.value();
    assert_eq!(value["content"][0]["content"][1]["text"], "bold");
    assert_eq!(
        value["content"][0]["content"][1]["marks"][0]["type"],
        "bold"
    );
}

#[test]
fn list_split_indent_and_outdent_keep_identity_and_ordered_start() {
    let doc = json!({"type":"doc","content":[{"type":"orderedList","attrs":{"start":7,"future":true},"content":[{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"one"}]}]},{"type":"listItem","content":[{"type":"paragraph","content":[{"type":"text","text":"two"}]}]}]}]});
    let mut editor = EditorModel::new(Document::parse(doc.to_string().into()).unwrap());
    editor.select(Selection::caret(10));
    let id = editor.document.resolve(10).unwrap().node.id;
    editor.indent_list(false).unwrap();
    assert_eq!(
        editor
            .document
            .resolve(editor.selection.head)
            .unwrap()
            .node
            .id,
        id
    );
    editor.indent_list(true).unwrap();
    assert_eq!(editor.document.root.value(), doc);
    assert_eq!(editor.selection.head, 10);
    editor.split_block().unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["attrs"]["start"],
        7
    );
    assert_eq!(
        editor.document.root.children.get(0).unwrap().children.len(),
        3
    );
}

#[test]
fn code_block_exit_and_readonly_protection() {
    let mut editor = model("code\n");
    editor.set_block("codeBlock", None).unwrap();
    editor.select(Selection::caret(6));
    editor.split_block().unwrap();
    assert_eq!(editor.document.root.children.len(), 2);
    assert_eq!(
        editor
            .document
            .resolve(editor.selection.head)
            .unwrap()
            .node
            .kind(),
        "paragraph"
    );
    editor.read_only = true;
    assert!(editor.replace(editor.selection.range(), "no").is_err());
    assert!(editor.undo().is_err());
    editor.select(Selection { anchor: 4, head: 1 });
    assert_eq!(editor.selection.range(), 1..4);
}

#[test]
fn mention_generation_rejects_stale_errors_and_dismissed_results() {
    let mut mentions = MentionResults::default();
    let stale = mentions.begin();
    let current = mentions.begin();
    assert!(!mentions.resolve(stale, Err("old".into())));
    let results = (0..10)
        .map(|n| MentionCandidate {
            label: n.to_string(),
            target: MentionTarget::Human(n.to_string()),
        })
        .collect();
    assert!(mentions.resolve(current, Ok(results)));
    assert_eq!(mentions.candidates.len(), 5);
    mentions.dismiss();
    assert!(!mentions.resolve(current, Err("late".into())));
    assert!(mentions.error.is_none());
}

#[test]
fn attachment_resize_preserves_identity_and_clamps_width() {
    let mut editor = EditorModel::new(Document::parse(json!({"type":"doc","content":[{"type":"image","attrs":{"attachmentId":"stable","editorWidth":80,"future":true}}]}).to_string().into()).unwrap());
    editor
        .update_node_attrs(
            &[0],
            serde_json::Map::from_iter([("editorWidth".into(), json!(130))]),
        )
        .unwrap();
    let attrs = editor.document.root.value()["content"][0]["attrs"].clone();
    assert_eq!(
        attrs,
        json!({"attachmentId":"stable","editorWidth":100.0,"future":true})
    );
    assert!(
        editor
            .update_node_attrs(
                &[0],
                serde_json::Map::from_iter([("path".into(), json!("/home/user/private"))])
            )
            .is_err()
    );
}

#[test]
fn projection_utf16_mapping_and_unknown_node_visibility() {
    let editor = model("a😀漢");
    let projected = ProjectedBlock::build(editor.document.root.children.get(0).unwrap().clone());
    let row = &projected.rows[0];
    assert_eq!(row.position(5), 4);
    assert_eq!(row.byte(4), Some(5));
    let document = Document::parse(r#"{"type":"doc","content":[{"type":"future-node","content":[{"type":"text","text":"retained"}]}]}"#.into()).unwrap();
    let projected = ProjectedBlock::build(document.root.children.get(0).unwrap().clone());
    assert!(projected.rows[0].text.contains("Unsupported future-node"));
}

#[test]
fn comment_range_mapping_drops_deleted_anchors() {
    let map = Mapping {
        old: 4..7,
        inserted: 0,
    };
    assert_eq!(map.map_anchor(4..7), None);
    assert_eq!(map.map_anchor(8..12), Some(5..9));
    assert_eq!(map.map_anchor(1..3), Some(1..3));
}

#[test]
fn native_text_range_uses_pm_structural_utf16_units() {
    let document = Document::parse(json!({"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"😀"}]},{"type":"paragraph","content":[{"type":"text","text":"漢"}]}]}).to_string().into()).unwrap();
    assert_eq!(
        clipboard::text_for_range(&document, 0..document.units()).unwrap(),
        "\n😀\n\n漢\n"
    );
    assert!(clipboard::text_for_range(&document, 1..2).is_err());
}

#[test]
fn punctuation_respects_code_context_and_keeps_one_history_step() {
    let mut editor = model("a-");
    editor.select(Selection::caret(3));
    editor.type_text(3..3, "-").unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["text"],
        "a—"
    );
    editor.undo().unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["text"],
        "a-"
    );
    editor.set_block("codeBlock", None).unwrap();
    editor.type_text(3..3, "-").unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["text"],
        "a--"
    );
    let mut editor = model("..");
    editor.type_text(3..3, ".").unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["text"],
        "…"
    );
}

#[test]
fn atomic_mention_and_attachment_insertions_are_portable_and_undoable() {
    let mut editor = model("@Jo");
    editor.select(Selection { anchor: 1, head: 4 });
    editor
        .insert_inline_atom(
            "mention-@",
            serde_json::Map::from_iter([
                ("id".into(), json!("person")),
                ("type".into(), json!("human")),
                ("label".into(), json!("John")),
            ]),
        )
        .unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["attrs"]["id"],
        "person"
    );
    editor
        .insert_block_atom(
            "image",
            serde_json::Map::from_iter([("attachmentId".into(), json!("attachment"))]),
        )
        .unwrap();
    assert_eq!(
        editor.document.root.value()["content"][1]["attrs"]["attachmentId"],
        "attachment"
    );
    assert_eq!(editor.document.root.children.len(), 3);
    editor.undo().unwrap();
    editor.undo().unwrap();
    assert_eq!(
        editor.document.root.value()["content"][0]["content"][0]["text"],
        "@Jo"
    );
    editor.select(Selection::caret(4));
    assert!(
        editor
            .insert_block_atom(
                "image",
                serde_json::Map::from_iter([("src".into(), json!("file:///private"))])
            )
            .is_err()
    );
}

#[test]
fn failed_structural_transaction_restores_text_selection_and_history() {
    let mut editor = model("/quote");
    editor.select(Selection::caret(7));
    let before = editor.document.serialize().unwrap();
    let revision = editor.revision;
    let result = editor.transaction(|model| {
        model.replace(1..7, "")?;
        Err("Service cannot allocate task identity".into())
    });
    assert!(result.is_err());
    assert_eq!(editor.revision, revision);
    assert_eq!(editor.document.serialize().unwrap(), before);
    assert_eq!(editor.selection, Selection::caret(7));
    editor
        .transaction(|model| {
            model.replace(1..7, "")?;
            model.wrap_block("blockquote")
        })
        .unwrap();
    editor.undo().unwrap();
    assert_eq!(editor.document.serialize().unwrap(), before);
}

#[test]
fn saved_attachments_match_portable_url_normalization() {
    let value = json!({"type":"doc","content":[
        {"type":"image","attrs":{"attachmentId":"id","src":"asset://localhost/private/file.png","path":"/private/file.png","editorWidth":80,"future":true}},
        {"type":"future-node","attrs":{"path":"opaque path preserved"}}
    ]});
    let document = Document::parse(value.to_string().into()).unwrap();
    let saved: Value = serde_json::from_str(&document.serialize_for_save().unwrap()).unwrap();
    assert_eq!(
        saved["content"][0]["attrs"],
        json!({"attachmentId":"id","editorWidth":80,"future":true})
    );
    assert_eq!(saved["content"][1], value["content"][1]);
    assert_eq!(document.root.value(), value);
    let document = Document::parse(json!({"type":"doc","content":[{"type":"image","attrs":{"src":"file:///private/file.png"}}]}).to_string().into()).unwrap();
    assert!(document.serialize_for_save().is_err());
}

#[test]
fn unicode_edits_match_a_flat_text_oracle() {
    let mut editor = model("seed😀");
    let mut expected = String::from("seed😀");
    let mut seed = 27u64;
    for step in 0..500 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut boundaries: Vec<_> = expected.char_indices().map(|(index, _)| index).collect();
        boundaries.push(expected.len());
        let a = (seed as usize) % boundaries.len();
        let b = ((seed >> 32) as usize) % boundaries.len();
        let start = boundaries[a.min(b)];
        let end = boundaries[a.max(b)];
        let insert = ["é", "👩‍💻", "漢字", "שלום", "", "x"][step % 6];
        let from = 1 + expected[..start].encode_utf16().count();
        let to = 1 + expected[..end].encode_utf16().count();
        editor.replace(from..to, insert).unwrap();
        expected.replace_range(start..end, insert);
        assert_eq!(
            super::document::inline_text(&editor.document.root.children.get(0).unwrap().children),
            expected
        );
    }
}

#[test]
#[ignore = "Run explicitly with --release --ignored --nocapture; editor model microbenchmark only"]
fn measured_editor_workload() {
    for count in [100, 1000, 10_000] {
        let fixture = json!({"type":"doc","content": (0..count).map(|i| json!({"type":"paragraph","content":[{"type":"text","text":format!("Block {i}: 中文 😀 é")}]})).collect::<Vec<_>>()});
        let bytes = fixture.to_string();
        for trial in 0..6 {
            let mut editor = EditorModel::new(Document::parse(bytes.clone().into()).unwrap());
            let before = editor
                .document
                .root
                .children
                .get(count - 1)
                .unwrap()
                .clone();
            let start = Instant::now();
            for _ in 0..2000 {
                editor.replace(1..1, "x").unwrap();
            }
            let edit_ns = start.elapsed().as_nanos();
            assert!(Arc::ptr_eq(
                &before,
                editor.document.root.children.get(count - 1).unwrap()
            ));
            let start = Instant::now();
            for _ in 0..2000 {
                editor.undo().unwrap();
            }
            let undo_ns = start.elapsed().as_nanos();
            assert_eq!(editor.document.root.value(), fixture);
            println!(
                "model_microbench blocks={count} trial={trial} warmup={} source_bytes={} edits=2000 edit_total_ns={edit_ns} undo_total_ns={undo_ns} unaffected_node_shared=true",
                trial == 0,
                bytes.len()
            );
        }
    }
}
