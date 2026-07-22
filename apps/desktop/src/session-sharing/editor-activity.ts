import { getAllWebviewWindows } from "@tauri-apps/api/webviewWindow";
import type { EditorView } from "prosemirror-view";

import { flushDatabaseWrites } from "~/db/write-queue";
import { useTabs } from "~/store/zustand/tabs";

const mountedCanonicalEditors = new Map<string, Map<EditorView, () => void>>();
const pendingEditorActivations = new Map<string, number>();
const importLocks = new Set<string>();
const importLockListeners = new Set<() => void>();

export function registerCanonicalSessionEditor(
  sessionId: string,
  view: EditorView,
  flushPendingChanges: () => void,
) {
  const editors = mountedCanonicalEditors.get(sessionId) ?? new Map();
  editors.set(view, flushPendingChanges);
  mountedCanonicalEditors.set(sessionId, editors);
}

export function unregisterCanonicalSessionEditor(
  sessionId: string,
  view: EditorView,
) {
  const editors = mountedCanonicalEditors.get(sessionId);
  if (!editors) return;
  editors.delete(view);
  if (editors.size === 0) mountedCanonicalEditors.delete(sessionId);
}

export async function flushCanonicalSessionEditorChanges(
  sessionId: string,
): Promise<void> {
  const editors = mountedCanonicalEditors.get(sessionId);
  if (editors) {
    for (const flushPendingChanges of editors.values()) {
      flushPendingChanges();
    }
  }
  await flushDatabaseWrites([`session:${sessionId}`]);
}

export async function isCanonicalSessionEditorActive(
  sessionId: string,
): Promise<boolean> {
  if (hasLocalSessionEditorActivity(sessionId)) return true;

  try {
    const windows = await getAllWebviewWindows();
    return windows.some((window) => window.label === `note-${sessionId}`);
  } catch {
    return true;
  }
}

export function tryAcquireCanonicalSessionImportLock(
  sessionId: string,
): (() => void) | null {
  if (importLocks.has(sessionId) || hasLocalSessionEditorActivity(sessionId)) {
    return null;
  }

  importLocks.add(sessionId);
  notifyImportLockListeners();
  let released = false;
  return () => {
    if (released) return;
    released = true;
    importLocks.delete(sessionId);
    notifyImportLockListeners();
  };
}

export function beginCanonicalSessionEditorActivation(
  sessionId: string,
): (() => void) | null {
  if (importLocks.has(sessionId)) return null;
  pendingEditorActivations.set(
    sessionId,
    (pendingEditorActivations.get(sessionId) ?? 0) + 1,
  );
  let released = false;
  return () => {
    if (released) return;
    released = true;
    const remaining = (pendingEditorActivations.get(sessionId) ?? 1) - 1;
    if (remaining > 0) {
      pendingEditorActivations.set(sessionId, remaining);
    } else {
      pendingEditorActivations.delete(sessionId);
    }
  };
}

export function isCanonicalSessionImportLocked(sessionId: string) {
  return importLocks.has(sessionId);
}

export function subscribeCanonicalSessionImportLocks(listener: () => void) {
  importLockListeners.add(listener);
  return () => importLockListeners.delete(listener);
}

export function waitForCanonicalSessionImportUnlock(
  sessionId: string,
): Promise<void> {
  if (!importLocks.has(sessionId)) return Promise.resolve();
  return new Promise((resolve) => {
    const unsubscribe = subscribeCanonicalSessionImportLocks(() => {
      if (importLocks.has(sessionId)) return;
      unsubscribe();
      resolve();
    });
  });
}

function hasLocalSessionEditorActivity(sessionId: string) {
  if ((mountedCanonicalEditors.get(sessionId)?.size ?? 0) > 0) return true;
  if ((pendingEditorActivations.get(sessionId) ?? 0) > 0) return true;
  const currentTab = useTabs.getState().currentTab;
  return currentTab?.type === "sessions" && currentTab.id === sessionId;
}

function notifyImportLockListeners() {
  for (const listener of importLockListeners) listener();
}
