import { useEffect, useState, type ReactNode } from "react";

import { LiveTranscriptFooter } from "./live-transcript";

import * as AudioPlayer from "~/audio-player";
import type { EditorView } from "~/store/zustand/tabs/schema";

export type SessionBottomAccessoryKind =
  | "live_transcript"
  | "live_transcript_expanded"
  | "playback";

export function useSessionBottomAccessory({
  sessionId,
  currentView,
  sessionMode,
  audioUrl,
  showConsentBanner,
}: {
  sessionId: string;
  currentView: EditorView;
  sessionMode: string;
  audioUrl: string | null | undefined;
  showConsentBanner: boolean;
}): {
  bottomAccessory: ReactNode;
  bottomAccessoryKind: SessionBottomAccessoryKind | null;
} {
  const [isExpanded, setIsExpanded] = useState(false);
  const kind = getSessionBottomAccessoryKind({
    currentView,
    sessionMode,
    audioUrl,
  });

  useEffect(() => {
    if (kind !== "live_transcript" && isExpanded) {
      setIsExpanded(false);
    }
  }, [isExpanded, kind]);

  if (kind === "live_transcript") {
    return {
      bottomAccessory: (
        <LiveTranscriptFooter
          sessionId={sessionId}
          showConsentBanner={showConsentBanner}
          isExpanded={isExpanded}
          onToggleExpand={() => setIsExpanded((value) => !value)}
        />
      ),
      bottomAccessoryKind: isExpanded ? "live_transcript_expanded" : kind,
    };
  }

  if (kind === "playback") {
    return {
      bottomAccessory: <AudioPlayer.Timeline />,
      bottomAccessoryKind: kind,
    };
  }

  return {
    bottomAccessory: null,
    bottomAccessoryKind: null,
  };
}

function getSessionBottomAccessoryKind({
  currentView,
  sessionMode,
  audioUrl,
}: {
  currentView: EditorView;
  sessionMode: string;
  audioUrl: string | null | undefined;
}): "live_transcript" | "playback" | null {
  if (sessionMode === "active" && currentView.type === "raw") {
    return "live_transcript";
  }

  if (
    currentView.type === "transcript" &&
    Boolean(audioUrl) &&
    sessionMode === "inactive"
  ) {
    return "playback";
  }

  return null;
}
