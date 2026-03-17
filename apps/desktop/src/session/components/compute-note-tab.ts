import type { EditorView } from "~/store/zustand/tabs/schema";

export function computeCurrentNoteTab(
  tabView: EditorView | null,
  isListenerActive: boolean,
  firstEnhancedNoteId: string | undefined,
): EditorView {
  if (isListenerActive) {
    if (tabView?.type === "raw" || tabView?.type === "transcript") {
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
