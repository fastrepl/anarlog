import type { EditorView } from "prosemirror-view";

import { flushDatabaseWrites } from "~/db/write-queue";

const mountedEditors = new Map<string, Map<EditorView, () => void>>();

export function registerSessionEditor(
  sessionId: string,
  view: EditorView,
  flushPendingChanges: () => void,
) {
  const editors = mountedEditors.get(sessionId) ?? new Map();
  editors.set(view, flushPendingChanges);
  mountedEditors.set(sessionId, editors);
}

export function unregisterSessionEditor(sessionId: string, view: EditorView) {
  const editors = mountedEditors.get(sessionId);
  if (!editors) return;
  editors.delete(view);
  if (editors.size === 0) mountedEditors.delete(sessionId);
}

export async function flushSessionEditorChanges(
  sessionId: string,
): Promise<void> {
  const editors = mountedEditors.get(sessionId);
  if (editors) {
    for (const flushPendingChanges of editors.values()) {
      flushPendingChanges();
    }
  }
  await flushDatabaseWrites([`session:${sessionId}`]);
}
