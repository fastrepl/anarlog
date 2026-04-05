import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { LiveTranscriptFooter } from "./live-transcript";
import { PostSessionAccessory } from "./post-session";

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

  const showPostSession = isInactive;
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
    () => (mode ? { mode, expanded: isExpanded } : null),
    [mode, isExpanded],
  );

  if (isLive) {
    return {
      bottomAccessory: (
        <LiveTranscriptFooter
          sessionId={sessionId}
          isExpanded={isExpanded}
          onToggleExpand={() => setIsExpanded((v) => !v)}
        />
      ),
      bottomAccessoryState,
    };
  }

  if (isFinalizing) {
    return {
      bottomAccessory: (
        <div className="relative w-full pt-3 select-none">
          <div className="rounded-xl bg-neutral-50">
            <div className="flex min-h-12 items-center gap-2 p-2">
              <div className="min-w-0 flex-1">
                <span className="text-xs text-neutral-400">Finalizing...</span>
              </div>
            </div>
          </div>
        </div>
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
