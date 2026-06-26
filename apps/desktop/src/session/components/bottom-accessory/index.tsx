import { useState, type ReactNode } from "react";

import { DuringSessionAccessory } from "./during-session";
import { ExpandToggle } from "./expand-toggle";

import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";

export type BottomAccessoryState = {
  mode: "playback" | "live";
  expanded: boolean;
} | null;

export function useSessionBottomAccessory(params: {
  sessionId: string;
  sessionMode: string;
  audioExists?: boolean;
  audioUrlReady?: boolean;
  isAudioLoading?: boolean;
  hasTranscript?: boolean;
}): {
  bottomAccessory: ReactNode;
  bottomBorderHandle: ReactNode;
  bottomAccessoryState: BottomAccessoryState;
} {
  const [liveExpanded, setLiveExpanded] = useState(false);
  const live = useListener((state) => ({
    status: state.live.status,
    sessionId: state.live.sessionId,
    requestedLiveTranscription: state.live.requestedLiveTranscription,
    liveTranscriptionActive: state.live.liveTranscriptionActive,
  }));

  const isCurrentLiveSession =
    params.sessionMode === "active" &&
    live.status === "active" &&
    live.sessionId === params.sessionId &&
    getLiveCaptureUiMode(live) === "live";

  if (isCurrentLiveSession) {
    return {
      bottomAccessory: (
        <DuringSessionAccessory
          sessionId={params.sessionId}
          isExpanded={liveExpanded}
        />
      ),
      bottomBorderHandle: (
        <ExpandToggle
          isExpanded={liveExpanded}
          onToggle={() => setLiveExpanded((value) => !value)}
          label="Live"
        />
      ),
      bottomAccessoryState: {
        mode: "live",
        expanded: liveExpanded,
      },
    };
  }

  return {
    bottomAccessory: null,
    bottomBorderHandle: null,
    bottomAccessoryState: null,
  };
}
