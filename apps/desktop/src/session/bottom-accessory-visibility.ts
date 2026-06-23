import type { BottomAccessoryState } from "./components/bottom-accessory";

import type { EditorView } from "~/store/zustand/tabs/schema";

export function shouldShowSessionBottomAccessory({
  currentView,
  bottomAccessoryState,
}: {
  currentView: EditorView;
  bottomAccessoryState: BottomAccessoryState;
}) {
  return (
    currentView.type !== "transcript" ||
    bottomAccessoryState?.mode === "playback" ||
    bottomAccessoryState?.mode === "transcript_only"
  );
}
