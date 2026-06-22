import type { EditorView } from "~/store/zustand/tabs/schema";

export function computeCurrentNoteTab(
  tabView: EditorView | null,
  isLiveSessionActive: boolean,
  firstEnhancedNoteId: string | undefined,
): EditorView {
  if (isLiveSessionActive) {
    if (tabView?.type === "raw" || tabView?.type === "transcript") {
      return tabView;
    }
    return { type: "raw" };
  }

  if (tabView) {
    if (tabView.type === "enhanced") {
      if (firstEnhancedNoteId && tabView.id === firstEnhancedNoteId) {
        return tabView;
      }

      if (firstEnhancedNoteId) {
        return { type: "enhanced", id: firstEnhancedNoteId };
      }

      return { type: "raw" };
    }

    if (tabView.type === "raw" || tabView.type === "transcript") {
      return tabView;
    }

    return { type: "raw" };
  }

  if (firstEnhancedNoteId) {
    return { type: "enhanced", id: firstEnhancedNoteId };
  }

  return { type: "raw" };
}

export function getPersistedNoteTabView(
  tabView: EditorView,
  isLiveSessionActive: boolean,
): EditorView {
  if (isLiveSessionActive && tabView.type === "enhanced") {
    return { type: "raw" };
  }

  return tabView;
}
