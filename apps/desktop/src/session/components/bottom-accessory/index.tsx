import { X } from "lucide-react";
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { DuringSessionAccessory } from "./during-session";
import { ExpandToggle } from "./expand-toggle";
import { shouldShowLiveTranscriptAccessory } from "./live-visibility";
import { usePastSessionNotes } from "./past-notes";
import { PostSessionAccessory, type PostSessionTab } from "./post-session";

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
  const [postSessionTab, setPostSessionTab] = useState<PostSessionTab | null>(
    null,
  );
  const isLive = sessionMode === "active";
  const isFinalizing = sessionMode === "finalizing";
  const isInactive = sessionMode === "inactive";
  const isRunningBatch = sessionMode === "running_batch";
  const hasAudio = Boolean(audioUrl) && (isInactive || isRunningBatch);
  const pastNotes = usePastSessionNotes(sessionId);
  const hasPastNotes = pastNotes.hasPastNotes;
  const generateMissingPastNotes = pastNotes.generateMissing;
  const activePostSessionTab: PostSessionTab = hasPastNotes
    ? (postSessionTab ??
      (!hasAudio && !hasTranscript && !isRunningBatch
        ? "past_notes"
        : "transcript"))
    : "transcript";
  const live = useListener((state) => state.live);
  const { chat } = useShell();
  const liveCaptureMode = getLiveCaptureUiMode(live);
  const shouldDeferToGlobalLiveAccessory =
    live.sessionId !== null &&
    live.sessionId !== sessionId &&
    shouldShowLiveTranscriptAccessory(live);
  const showLiveAccessory =
    !shouldDeferToGlobalLiveAccessory && isLive && liveCaptureMode === "live";
  const canExpandLiveTranscript = showLiveAccessory;
  const effectiveExpanded =
    isLive && !canExpandLiveTranscript ? false : isExpanded;
  const isChatVisible =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";

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
    isRunningBatch ||
    (!shouldDeferToGlobalLiveAccessory &&
      isInactive &&
      (hasAudio || hasTranscript || hasPastNotes));
  const selectPostSessionTab = useCallback(
    (tab: PostSessionTab) => {
      const shouldExpand = activePostSessionTab !== tab || !isExpanded;
      if (tab === "past_notes" && shouldExpand) {
        generateMissingPastNotes();
      }

      setPostSessionTab(tab);
      setIsExpanded((expanded) =>
        activePostSessionTab === tab ? !expanded : true,
      );
    },
    [activePostSessionTab, generateMissingPastNotes, isExpanded],
  );

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

  const mode: NonNullable<BottomAccessoryState>["mode"] | null =
    showLiveAccessory
      ? "live"
      : isFinalizing
        ? "finalizing"
        : showPostSession
          ? hasAudio || isRunningBatch
            ? "playback"
            : "transcript_only"
          : null;

  const bottomAccessoryState: BottomAccessoryState = useMemo(
    () => (mode ? { mode, expanded: effectiveExpanded } : null),
    [effectiveExpanded, mode],
  );

  if (showLiveAccessory || isFinalizing) {
    return {
      bottomAccessory: (
        <DuringSessionAccessory
          sessionId={sessionId}
          isFinalizing={isFinalizing}
          isExpanded={effectiveExpanded}
          fillHeight={effectiveExpanded && !isFinalizing}
        />
      ),
      bottomBorderHandle:
        canExpandLiveTranscript && !isFinalizing ? (
          <ExpandToggle
            isExpanded={effectiveExpanded}
            onToggle={() => setIsExpanded((v) => !v)}
            label="Live"
            collapsedClassName="bg-neutral-50"
            expandedClassName="bg-neutral-50"
          />
        ) : null,
      bottomAccessoryState,
    };
  }

  if (showPostSession) {
    const hasAccessoryContent = isExpanded || hasAudio || isRunningBatch;
    return {
      bottomAccessory: hasAccessoryContent ? (
        <PostSessionAccessory
          sessionId={sessionId}
          hasAudio={hasAudio}
          hasTranscript={hasTranscript}
          isTranscriptExpanded={isExpanded}
          activeTab={activePostSessionTab}
          pastNotes={pastNotes.notes}
          fillHeight={isExpanded}
        />
      ) : null,
      bottomBorderHandle: hasPastNotes ? (
        <PostSessionTabHandle
          isExpanded={isExpanded}
          activeTab={activePostSessionTab}
          onSelect={selectPostSessionTab}
        />
      ) : (
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

function PostSessionTabHandle({
  isExpanded,
  activeTab,
  onSelect,
}: {
  isExpanded: boolean;
  activeTab: PostSessionTab;
  onSelect: (tab: PostSessionTab) => void;
}) {
  return (
    <div className="relative left-3 z-10 flex h-5 items-center">
      <PostSessionTabButton
        label="Transcript"
        tab="transcript"
        activeTab={activeTab}
        isExpanded={isExpanded}
        onSelect={onSelect}
        className="rounded-tl-[10px] border-l"
      />
      <PostSessionTabButton
        label="Past notes"
        tab="past_notes"
        activeTab={activeTab}
        isExpanded={isExpanded}
        onSelect={onSelect}
        className="-ml-px rounded-tr-[10px] border-r"
      />
    </div>
  );
}

function PostSessionTabButton({
  label,
  tab,
  activeTab,
  isExpanded,
  onSelect,
  className,
}: {
  label: string;
  tab: PostSessionTab;
  activeTab: PostSessionTab;
  isExpanded: boolean;
  onSelect: (tab: PostSessionTab) => void;
  className?: string;
}) {
  const isActive = activeTab === tab;

  return (
    <button
      type="button"
      onClick={() => onSelect(tab)}
      className={cn([
        "relative flex h-5 items-center justify-center gap-1 border-t border-neutral-200 px-3",
        "after:pointer-events-none after:absolute after:right-px after:-bottom-px after:left-px after:h-0.5 after:bg-inherit after:content-['']",
        "text-[10px] font-medium transition-colors",
        isActive && isExpanded
          ? "bg-neutral-50 text-neutral-600"
          : "bg-white text-neutral-400",
        "hover:cursor-pointer hover:bg-neutral-100 hover:text-neutral-600",
        className,
      ])}
      aria-label={
        isActive && isExpanded ? `Collapse ${label}` : `Expand ${label}`
      }
    >
      <span>{label}</span>
      {isActive && isExpanded ? <X size={10} className="shrink-0" /> : null}
    </button>
  );
}
