import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { DuringSessionAccessory } from "./during-session";
import { PostSessionAccessory } from "./post-session";

import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";

export type BottomAccessoryState = {
  mode: "live" | "playback" | "transcript_only" | "finalizing";
  expanded: boolean;
} | null;

export function useSessionBottomAccessory({
  sessionId,
  sessionMode,
  audioUrl,
  hasTranscript,
}: {
  sessionId: string;
  sessionMode: string;
  audioUrl: string | null | undefined;
  hasTranscript: boolean;
}): {
  bottomAccessory: ReactNode;
  bottomAccessoryState: BottomAccessoryState;
} {
  const [isExpanded, setIsExpanded] = useState(false);
  const isLive = sessionMode === "active";
  const isFinalizing = sessionMode === "finalizing";
  const isBatching = sessionMode === "running_batch";
  const isInactive = sessionMode === "inactive" || isBatching;
  const hasAudio = Boolean(audioUrl) && isInactive;
  const live = useListener((state) => state.live);
  const liveCaptureMode = getLiveCaptureUiMode(live);
  const canExpandLiveTranscript = isLive && liveCaptureMode === "live";
  const effectiveExpanded =
    isLive && !canExpandLiveTranscript ? false : isExpanded;

  const prevLive = useRef(isLive);
  useEffect(() => {
    if (prevLive.current && !isLive) {
      setIsExpanded(false);
    }
    prevLive.current = isLive;
  }, [isLive]);

  useEffect(() => {
    if (isLive && !canExpandLiveTranscript && isExpanded) {
      setIsExpanded(false);
    }
  }, [isLive, canExpandLiveTranscript, isExpanded]);

  const showPostSession =
    isInactive && (isBatching || hasAudio || hasTranscript);
  const mode: NonNullable<BottomAccessoryState>["mode"] | null = isLive
    ? "live"
    : isFinalizing
      ? "finalizing"
      : showPostSession
        ? hasAudio
          ? "playback"
          : "transcript_only"
        : null;

  const bottomAccessoryState: BottomAccessoryState = useMemo(
    () => (mode ? { mode, expanded: effectiveExpanded } : null),
    [effectiveExpanded, mode],
  );

  if (isLive || isFinalizing) {
    return {
      bottomAccessory: (
        <DuringSessionAccessory
          sessionId={sessionId}
          isFinalizing={isFinalizing}
          isExpanded={effectiveExpanded}
          onToggleExpand={
            canExpandLiveTranscript ? () => setIsExpanded((v) => !v) : undefined
          }
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
