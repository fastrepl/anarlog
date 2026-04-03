import type { EditorView } from "~/store/zustand/tabs/schema";

export function computeCurrentNoteTab(
  tabView: EditorView | null,
  isLiveSessionActive: boolean,
  firstEnhancedNoteId: string | undefined,
): EditorView {
  if (isLiveSessionActive) {
    if (tabView?.type === "raw") {
      return tabView;
    }
    return { type: "raw" };
  }

  if (tabView) {
    return tabView;
  }

  if (firstEnhancedNoteId) {
    return { type: "enhanced", id: firstEnhancedNoteId };
  }

  return { type: "raw" };
}
