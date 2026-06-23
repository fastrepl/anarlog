import type { EditorView } from "~/store/zustand/tabs/schema";

export function computeCurrentNoteTab(
  tabView: EditorView | null,
  isLiveSessionActive: boolean,
  firstEnhancedNoteId: string | undefined,
  canShowTranscript = false,
): EditorView {
  if (isLiveSessionActive) {
    if (tabView?.type === "raw" || tabView?.type === "transcript") {
      return tabView;
    }
    if (tabView?.type === "enhanced" && firstEnhancedNoteId) {
      return tabView;
    }
    return { type: "raw" };
  }

  if (tabView) {
    if (tabView.type === "raw") {
      return tabView;
    }
    if (tabView.type === "enhanced" && firstEnhancedNoteId) {
      return tabView;
    }
    if (tabView.type === "transcript" && canShowTranscript) {
      return tabView;
    }

    return { type: "raw" };
  }

  if (firstEnhancedNoteId) {
    return { type: "enhanced", id: firstEnhancedNoteId };
  }

  return { type: "raw" };
}
