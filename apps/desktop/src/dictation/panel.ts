import { useDictationStatus } from "./state";

import type { FloatingRouteState } from "~/meeting-float/route-state";
import { DEFAULT_FLOATING_OVERLAY_SETTINGS } from "~/meeting-float/settings";

const emptyBubbles: FloatingRouteState["transcriptBubbles"] = [];

export function waitForDictationPanel(
  owner: string,
  signal: AbortSignal,
): Promise<void> {
  signal.throwIfAborted();
  if (useDictationStatus.getState().presentedOwner === owner)
    return Promise.resolve();
  return new Promise((resolve, reject) => {
    const finish = (error?: Error) => {
      clearTimeout(timeout);
      unsubscribe();
      signal.removeEventListener("abort", abort);
      if (error) reject(error);
      else resolve();
    };
    const abort = () => finish(new Error("Dictation cancelled"));
    const timeout = setTimeout(
      () =>
        finish(
          new Error("Could not open the dictation panel. Please try again."),
        ),
      5_000,
    );
    const unsubscribe = useDictationStatus.subscribe((state) => {
      if (state.presentedOwner === owner) finish();
    });
    signal.addEventListener("abort", abort, { once: true });
  });
}

export function getDictationPanelState(): FloatingRouteState | null {
  const state = useDictationStatus.getState();
  if (state.phase === "idle" || !state.owner) return null;
  return {
    sessionId: state.owner,
    title: "Dictation",
    amplitude: state.phase === "recording" ? state.amplitude : 0,
    status: "recording",
    colorScheme: "dark",
    opacity: 1,
    liveCaptionOpacity: 1,
    liveCaptionWidth: 360,
    liveCaptionLineCount: 4,
    liveCaptionPosition: DEFAULT_FLOATING_OVERLAY_SETTINGS.liveCaptionPosition,
    liveCaptionMinimized: !state.expanded,
    liveCaptionToggleVisible: true,
    transcriptBubbles: emptyBubbles,
    dictation: {
      sessionId: state.owner,
      phase: state.phase,
      microphone: state.microphone,
      text: state.text,
      partial: state.partial,
      previewEnabled: state.previewEnabled,
      previewUnavailable: state.previewUnavailable,
    },
  };
}
