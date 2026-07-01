import { EditorState, TextSelection } from "prosemirror-state";
import type { EditorView } from "prosemirror-view";
import { afterEach, describe, expect, test } from "vitest";

import { schema } from "@hypr/editor/note";

import {
  appendRawNoteParagraphs,
  getRawNoteEditorContent,
  registerRawNoteEditor,
  unregisterRawNoteEditor,
} from "./raw-note-registry";

describe("raw note registry", () => {
  let registeredView: EditorView | null = null;

  afterEach(() => {
    if (registeredView) {
      unregisterRawNoteEditor("session-1", registeredView);
      registeredView = null;
    }
  });

  test("appends without losing unsaved editor content or moving the selection", () => {
    const doc = schema.node("doc", null, [
      schema.node("heading", { level: 1 }, schema.text("Weekly sync")),
      schema.node("paragraph", null, schema.text("Unsaved decision")),
    ]);
    const view = createEditorView(
      EditorState.create({
        schema,
        doc,
        selection: TextSelection.create(doc, 3),
      }),
    );
    registeredView = view;
    registerRawNoteEditor("session-1", view);
    const selectionBefore = view.state.selection.from;

    const result = appendRawNoteParagraphs("session-1", [
      {
        type: "paragraph",
        content: [{ type: "text", text: "[Zoom chat] Ada: Ship it" }],
      },
    ]);

    expect(result.status).toBe("updated");
    expect(view.state.doc.textContent).toContain("Unsaved decision");
    expect(view.state.doc.textContent).toContain("[Zoom chat] Ada: Ship it");
    expect(view.state.selection.from).toBe(selectionBefore);
    expect(getRawNoteEditorContent("session-1")).toEqual(
      view.state.doc.toJSON(),
    );
  });

  test("defers capture while the editor is composing", () => {
    const view = createEditorView(EditorState.create({ schema }), true);
    registeredView = view;
    registerRawNoteEditor("session-1", view);

    expect(
      appendRawNoteParagraphs("session-1", [
        {
          type: "paragraph",
          content: [{ type: "text", text: "Deferred" }],
        },
      ]),
    ).toEqual({ status: "deferred" });
    expect(view.state.doc.textContent).toBe("");
  });
});

function createEditorView(
  initialState: EditorState,
  composing = false,
): EditorView {
  let state = initialState;
  return {
    get state() {
      return state;
    },
    composing,
    isDestroyed: false,
    dispatch(transaction) {
      state = state.apply(transaction);
    },
  } as EditorView;
}
