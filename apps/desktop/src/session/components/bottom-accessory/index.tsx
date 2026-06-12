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

const BOTTOM_ACCESSORY_HANDLE_CLASS = "bg-card dark:bg-app-floating-chrome";

export type BottomAccessoryState = {
  mode: "live" | "playback" | "transcript_only";
  expanded: boolean;
} | null;

export function useSessionBottomAccessory({
  sessionId,
  sessionMode,
  audioExists,
  audioUrlReady = audioExists,
  isAudioLoading = false,
  hasTranscript,
}: {
  sessionId: string;
  sessionMode: string;
  audioExists: boolean;
  audioUrlReady?: boolean;
  isAudioLoading?: boolean;
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
  const isInactive = sessionMode === "inactive";
  const isRunningBatch = sessionMode === "running_batch";
  const canUsePlayback = audioUrlReady && (isInactive || isRunningBatch);
  const mayHaveAudio =
    (audioExists || isAudioLoading) && (isInactive || isRunningBatch);
  const batchError = useListener(
    (state) => state.batch[sessionId]?.error ?? null,
  );
  const hasTranscriptError = Boolean(batchError);
  const canShowTranscriptTab =
    canUsePlayback || hasTranscript || hasTranscriptError || isRunningBatch;
  const pastNotes = usePastSessionNotes(sessionId);
  const hasPastNotes = pastNotes.hasPastNotes;
  const generateMissingPastNotes = pastNotes.generateMissing;
  const regeneratePastNote = pastNotes.canGenerate
    ? pastNotes.regenerate
    : undefined;
  const activePostSessionTab: PostSessionTab =
    hasPastNotes && !canShowTranscriptTab
      ? "past_notes"
      : hasPastNotes
        ? (postSessionTab ?? "transcript")
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
      (mayHaveAudio || hasTranscript || hasTranscriptError || hasPastNotes));
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
      : showPostSession
        ? canUsePlayback || isRunningBatch
          ? "playback"
          : "transcript_only"
        : null;

  const bottomAccessoryState: BottomAccessoryState = useMemo(
    () => (mode ? { mode, expanded: effectiveExpanded } : null),
    [effectiveExpanded, mode],
  );

  if (showLiveAccessory) {
    return {
      bottomAccessory: (
        <DuringSessionAccessory
          sessionId={sessionId}
          isExpanded={effectiveExpanded}
          fillHeight={effectiveExpanded}
        />
      ),
      bottomBorderHandle: canExpandLiveTranscript ? (
        <ExpandToggle
          isExpanded={effectiveExpanded}
          onToggle={() => setIsExpanded((v) => !v)}
          label="Live"
          collapsedClassName={BOTTOM_ACCESSORY_HANDLE_CLASS}
          expandedClassName={BOTTOM_ACCESSORY_HANDLE_CLASS}
        />
      ) : null,
      bottomAccessoryState,
    };
  }

  if (showPostSession) {
    const hasAccessoryContent = isExpanded || isRunningBatch;
    return {
      bottomAccessory: hasAccessoryContent ? (
        <PostSessionAccessory
          sessionId={sessionId}
          hasAudio={canUsePlayback}
          hasTranscript={hasTranscript}
          isTranscriptExpanded={isExpanded}
          activeTab={activePostSessionTab}
          pastNotes={pastNotes.notes}
          onRegeneratePastNote={regeneratePastNote}
          fillHeight={isExpanded}
        />
      ) : null,
      bottomBorderHandle: hasPastNotes ? (
        <PostSessionTabHandle
          isExpanded={isExpanded}
          activeTab={activePostSessionTab}
          showTranscriptTab={canShowTranscriptTab}
          onSelect={selectPostSessionTab}
        />
      ) : (
        <ExpandToggle
          isExpanded={isExpanded}
          onToggle={() => setIsExpanded((v) => !v)}
          label="Transcript"
          showExpandedCloseIcon
          collapsedClassName={BOTTOM_ACCESSORY_HANDLE_CLASS}
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
  showTranscriptTab,
  onSelect,
}: {
  isExpanded: boolean;
  activeTab: PostSessionTab;
  showTranscriptTab: boolean;
  onSelect: (tab: PostSessionTab) => void;
}) {
  return (
    <div className="relative left-3 z-10 flex h-5 items-center gap-1">
      {showTranscriptTab ? (
        <PostSessionTabButton
          label="Transcript"
          tab="transcript"
          activeTab={activeTab}
          isExpanded={isExpanded}
          onSelect={onSelect}
          className="rounded-t-[10px] border-x"
        />
      ) : null}
      <PostSessionTabButton
        label="Related meetings"
        tab="past_notes"
        activeTab={activeTab}
        isExpanded={isExpanded}
        onSelect={onSelect}
        className="rounded-t-[10px] border-x"
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
        "border-border relative flex h-5 items-center justify-center gap-1 border-t px-3",
        "after:pointer-events-none after:absolute after:right-px after:-bottom-px after:left-px after:h-0.5 after:bg-inherit after:content-['']",
        "text-[10px] font-medium transition-colors",
        isActive && isExpanded
          ? "bg-card text-foreground"
          : "bg-card text-muted-foreground",
        "hover:bg-accent hover:text-muted-foreground hover:cursor-pointer",
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
