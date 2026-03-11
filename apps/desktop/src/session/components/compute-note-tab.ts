import type { EditorView } from "~/store/zustand/tabs/schema";

export function computeCurrentNoteTab(
  tabView: EditorView | null,
  isListenerActive: boolean,
  hasTranscript: boolean,
  enhancedNoteIds: string[],
): EditorView {
  if (isListenerActive) {
    if (tabView?.type === "raw" || tabView?.type === "transcript") {
      return tabView;
    }
    return { type: "raw" };
  }

  if (tabView) {
    if (tabView.type === "transcript" && !hasTranscript) {
      tabView = null;
    } else if (
      tabView.type === "enhanced" &&
      !enhancedNoteIds.includes(tabView.id)
    ) {
      tabView = null;
    } else {
      return tabView;
    }
  }

  const firstEnhancedNoteId = enhancedNoteIds[0];
  if (firstEnhancedNoteId) {
    return { type: "enhanced", id: firstEnhancedNoteId };
  }

  return { type: "raw" };
}
