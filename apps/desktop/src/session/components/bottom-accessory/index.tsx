import { X } from "lucide-react";
import { useCallback, useMemo, useState, type ReactNode } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { shouldShowLiveTranscriptAccessory } from "./live-visibility";
import { usePastSessionNotes } from "./past-notes";
import { PostSessionAccessory, type PostSessionTab } from "./post-session";

import { useShell } from "~/contexts/shell";
import { useListener } from "~/stt/contexts";

export type BottomAccessoryState = {
  mode: "playback" | "transcript_only";
  expanded: boolean;
} | null;

export function useSessionBottomAccessory({
  sessionId,
  sessionMode,
}: {
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
  const [isExpanded, setIsExpanded] = useState(false);
  const [postSessionTab, setPostSessionTab] = useState<PostSessionTab | null>(
    null,
  );
  const isInactive = sessionMode === "inactive";
  const isRunningBatch = sessionMode === "running_batch";
  const live = useListener((state) => ({
    status: state.live.status,
    sessionId: state.live.sessionId,
    requestedLiveTranscription: state.live.requestedLiveTranscription,
    liveTranscriptionActive: state.live.liveTranscriptionActive,
  }));
  const { chat } = useShell();
  const shouldDeferToGlobalLiveAccessory =
    live.sessionId !== null &&
    live.sessionId !== sessionId &&
    shouldShowLiveTranscriptAccessory(live);
  const shouldLoadPastNotes =
    (isInactive || isRunningBatch) && !shouldDeferToGlobalLiveAccessory;
  const pastNotes = usePastSessionNotes(sessionId, {
    enabled: shouldLoadPastNotes,
  });
  const hasPastNotes = pastNotes.hasPastNotes;
  const generateMissingPastNotes = pastNotes.generateMissing;
  const activePostSessionTab: PostSessionTab = hasPastNotes
    ? (postSessionTab ?? "insights")
    : "transcript";
  const isChatVisible =
    chat.mode === "FloatingOpen" || chat.mode === "RightPanelOpen";

  const showPostSession =
    isRunningBatch ||
    (!shouldDeferToGlobalLiveAccessory && isInactive && hasPastNotes);
  const selectPostSessionTab = useCallback(
    (tab: PostSessionTab) => {
      const shouldExpand = activePostSessionTab !== tab || !isExpanded;
      if (tab === "insights" && shouldExpand) {
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

  const mode: NonNullable<BottomAccessoryState>["mode"] | null = showPostSession
    ? isRunningBatch
      ? "playback"
      : "transcript_only"
    : null;

  const bottomAccessoryState: BottomAccessoryState = useMemo(
    () => (mode ? { mode, expanded: isExpanded } : null),
    [isExpanded, mode],
  );

  if (showPostSession) {
    const hasAccessoryContent = isRunningBatch || (hasPastNotes && isExpanded);
    return {
      bottomAccessory: hasAccessoryContent ? (
        <PostSessionAccessory
          sessionId={sessionId}
          isTranscriptExpanded={isExpanded}
          activeTab={activePostSessionTab}
          pastNotes={pastNotes.notes}
          onRegenerateInsights={
            pastNotes.canGenerate ? pastNotes.regenerateAll : undefined
          }
          fillHeight={isExpanded}
        />
      ) : null,
      bottomBorderHandle: hasPastNotes ? (
        <PostSessionTabHandle
          isExpanded={isExpanded}
          activeTab={activePostSessionTab}
          showTranscriptTab={false}
          onSelect={selectPostSessionTab}
        />
      ) : null,
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
        label="Insights"
        tab="insights"
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
        "after:pointer-events-none after:absolute after:right-px after:-bottom-0.5 after:left-px after:h-1 after:bg-inherit after:content-['']",
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
