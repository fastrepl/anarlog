import { Fragment } from "prosemirror-model";
import type { EditorView } from "prosemirror-view";

import type { JSONContent } from "@hypr/editor/note";

const rawNoteEditors = new Map<string, EditorView>();

export function registerRawNoteEditor(sessionId: string, view: EditorView) {
  rawNoteEditors.set(sessionId, view);
}

export function unregisterRawNoteEditor(sessionId: string, view: EditorView) {
  if (rawNoteEditors.get(sessionId) === view) {
    rawNoteEditors.delete(sessionId);
  }
}

export function getRawNoteEditorContent(sessionId: string): JSONContent | null {
  const view = getRawNoteEditor(sessionId);
  return view ? view.state.doc.toJSON() : null;
}

export function appendRawNoteParagraphs(
  sessionId: string,
  paragraphs: JSONContent[],
):
  | { status: "updated"; rawMd: string }
  | { status: "deferred" | "unavailable" } {
  const view = getRawNoteEditor(sessionId);
  if (!view) {
    return { status: "unavailable" };
  }

  if (view.composing) {
    return { status: "deferred" };
  }

  const nodes = paragraphs.map((paragraph) =>
    view.state.schema.nodeFromJSON(paragraph),
  );
  if (nodes.length === 0) {
    return {
      status: "updated",
      rawMd: JSON.stringify(view.state.doc.toJSON()),
    };
  }

  const transaction = view.state.tr
    .insert(view.state.doc.content.size, Fragment.fromArray(nodes))
    .setMeta("addToHistory", false);
  const rawMd = JSON.stringify(transaction.doc.toJSON());
  view.dispatch(transaction);

  return { status: "updated", rawMd };
}

function getRawNoteEditor(sessionId: string) {
  const view = rawNoteEditors.get(sessionId);
  if (view?.isDestroyed) {
    rawNoteEditors.delete(sessionId);
    return null;
  }

  return view ?? null;
}
