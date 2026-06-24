import type { BottomAccessoryState } from "./components/bottom-accessory";

import type { EditorView } from "~/store/zustand/tabs/schema";

export function shouldShowSessionBottomAccessory({
  currentView,
  bottomAccessoryState,
  sessionMode,
}: {
  currentView: EditorView;
  bottomAccessoryState: BottomAccessoryState;
  sessionMode: string;
}) {
  if (sessionMode === "running_batch") {
    return bottomAccessoryState?.mode === "playback";
  }

  return (
    currentView.type !== "transcript" ||
    bottomAccessoryState?.mode === "playback" ||
    bottomAccessoryState?.mode === "transcript_only"
  );
}
