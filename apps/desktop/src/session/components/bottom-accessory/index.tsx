import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { DuringSessionAccessory } from "./during-session";
import { ExpandToggle } from "./expand-toggle";
import { PostSessionAccessory } from "./post-session";

import { useShell } from "~/contexts/shell";
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
  bottomBorderHandle: ReactNode;
  bottomAccessoryState: BottomAccessoryState;
} {
  const [isExpanded, setIsExpanded] = useState(false);
  const isLive = sessionMode === "active";
  const isFinalizing = sessionMode === "finalizing";
  const isBatching = sessionMode === "running_batch";
  const isInactive = sessionMode === "inactive" || isBatching;
  const hasAudio = Boolean(audioUrl) && isInactive;
  const live = useListener((state) => state.live);
  const { chat } = useShell();
  const liveCaptureMode = getLiveCaptureUiMode(live);
  const canExpandLiveTranscript = isLive && liveCaptureMode === "live";
  const effectiveExpanded =
    isLive && !canExpandLiveTranscript ? false : isExpanded;
  const isChatVisible = chat.mode === "RightPanelOpen";

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

  useHotkeys(
    "esc",
    () => {
      setIsExpanded(false);
    },
    {
      enabled: showPostSession && isExpanded && !isChatVisible,
      preventDefault: true,
      enableOnFormTags: true,
      enableOnContentEditable: true,
    },
    [showPostSession, isExpanded, isChatVisible],
  );

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
        />
      ),
      bottomBorderHandle:
        canExpandLiveTranscript && !isFinalizing ? (
          <ExpandToggle
            isExpanded={effectiveExpanded}
            onToggle={() => setIsExpanded((v) => !v)}
            label="Live"
            collapsedClassName="bg-neutral-50"
          />
        ) : null,
      bottomAccessoryState,
    };
  }

  if (showPostSession) {
    const hasAccessoryContent = isExpanded || isBatching;
    return {
      bottomAccessory: hasAccessoryContent ? (
        <PostSessionAccessory
          sessionId={sessionId}
          hasAudio={hasAudio}
          hasTranscript={hasTranscript}
          isTranscriptExpanded={isExpanded}
        />
      ) : null,
      bottomBorderHandle: (
        <ExpandToggle
          isExpanded={isExpanded}
          onToggle={() => setIsExpanded((v) => !v)}
          label="Transcript"
          showExpandedCloseIcon
          collapsedClassName="bg-neutral-50"
        />
      ),
      bottomAccessoryState,
    };
  }

  return {
    bottomAccessory: null,
    bottomBorderHandle: null,
    bottomAccessoryState,
  };
}
