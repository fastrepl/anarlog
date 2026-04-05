import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { LiveTranscriptFooter } from "./live-transcript";
import { PostSessionAccessory } from "./post-session";

export type BottomAccessoryState = {
  mode: "live" | "playback" | "transcript_only";
  expanded: boolean;
} | null;

export function useSessionBottomAccessory({
  sessionId,
  sessionMode,
  audioUrl,
  showConsentBanner,
  hasTranscript,
}: {
  sessionId: string;
  sessionMode: string;
  audioUrl: string | null | undefined;
  showConsentBanner: boolean;
  hasTranscript: boolean;
}): {
  bottomAccessory: ReactNode;
  bottomAccessoryState: BottomAccessoryState;
} {
  const [isExpanded, setIsExpanded] = useState(false);
  const isLive = sessionMode === "active";
  const isInactive =
    sessionMode === "inactive" || sessionMode === "running_batch";
  const isBatching = sessionMode === "running_batch";
  const hasAudio = Boolean(audioUrl) && isInactive;

  const prevLive = useRef(isLive);
  useEffect(() => {
    if (prevLive.current && !isLive) {
      setIsExpanded(false);
    }
    prevLive.current = isLive;
  }, [isLive]);

  useEffect(() => {
    if (isBatching) {
      setIsExpanded(true);
    }
  }, [isBatching]);

  const showPostSession = isInactive && (hasAudio || hasTranscript);
  const mode: NonNullable<BottomAccessoryState>["mode"] | null = isLive
    ? "live"
    : showPostSession
      ? hasAudio
        ? "playback"
        : "transcript_only"
      : null;

  const bottomAccessoryState: BottomAccessoryState = useMemo(
    () => (mode ? { mode, expanded: isExpanded } : null),
    [mode, isExpanded],
  );

  if (isLive) {
    return {
      bottomAccessory: (
        <LiveTranscriptFooter
          sessionId={sessionId}
          showConsentBanner={showConsentBanner}
          isExpanded={isExpanded}
          onToggleExpand={() => setIsExpanded((v) => !v)}
        />
      ),
      bottomAccessoryState,
    };
  }

  if (showPostSession) {
    return {
      bottomAccessory: (
        <PostSessionAccessory
          sessionId={sessionId}
          hasAudio={hasAudio}
          hasTranscript={hasTranscript}
          isTranscriptExpanded={isExpanded}
          onToggleTranscript={() => setIsExpanded((v) => !v)}
        />
      ),
      bottomAccessoryState,
    };
  }

  return {
    bottomAccessory: null,
    bottomAccessoryState,
  };
}
