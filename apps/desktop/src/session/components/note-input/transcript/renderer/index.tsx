import { ArrowDownIcon, ArrowUpIcon } from "lucide-react";
import {
  type RefObject,
  useCallback,
  useDeferredValue,
  useRef,
  useState,
} from "react";
import { useHotkeys } from "react-hotkeys-hook";

import { cn } from "@hypr/utils";

import { SelectionMenu } from "./selection-menu";
import { TranscriptSeparator } from "./separator";
import { RenderTranscript } from "./transcript";
import {
  useAutoScroll,
  usePlaybackAutoScroll,
  useScrollDetection,
} from "./viewport-hooks";

import { useAudioPlayer } from "~/audio-player";
import { useAudioTime } from "~/audio-player/provider";
import { useShell } from "~/contexts/shell";
import type { Segment } from "~/stt/live-segment";

const LIVE_TRANSCRIPT_PLACEHOLDER_ID = "__live-transcript__";

export function TranscriptViewer({
  transcriptIds,
  liveSegments,
  currentActive,
  scrollRef,
}: {
  transcriptIds: string[];
  liveSegments: Segment[];
  currentActive: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const { chat } = useShell();
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollElement, setScrollElement] = useState<HTMLDivElement | null>(
    null,
  );
  const handleContainerRef = useCallback(
    (node: HTMLDivElement | null) => {
      containerRef.current = node;
      setScrollElement(node);
      scrollRef.current = node;
    },
    [scrollRef],
  );

  const {
    isAtTop,
    isAtBottom,
    autoScrollEnabled,
    scrollTarget,
    scrollToTop,
    scrollToBottom,
  } = useScrollDetection(containerRef, currentActive);

  const {
    state: playerState,
    pause,
    resume,
    start,
    seek,
    audioExists,
  } = useAudioPlayer();
  const time = useAudioTime();
  const deferredCurrentMs = useDeferredValue(time.current * 1000);
  const isPlaying = playerState === "playing";

  useHotkeys(
    "space",
    (e) => {
      e.preventDefault();
      if (playerState === "playing") {
        pause();
      } else if (playerState === "paused") {
        resume();
      } else if (playerState === "stopped") {
        start();
      }
    },
    { enableOnFormTags: false },
  );

  usePlaybackAutoScroll(containerRef, deferredCurrentMs, isPlaying);
  const shouldAutoScroll = currentActive && autoScrollEnabled;
  const shouldScrollLastTranscriptToEnd = currentActive && isAtBottom;
  useAutoScroll(
    containerRef,
    [transcriptIds, liveSegments, shouldAutoScroll],
    shouldAutoScroll,
  );
  const visibleTranscriptIds =
    transcriptIds.length > 0
      ? transcriptIds
      : liveSegments.length > 0
        ? [LIVE_TRANSCRIPT_PLACEHOLDER_ID]
        : [];

  const canShowScrollChip = !currentActive && (!isAtTop || !isAtBottom);
  const scrollChip =
    chat.mode === "FloatingOpen" || !canShowScrollChip
      ? null
      : scrollTarget === "bottom" && !isAtBottom
        ? {
            icon: ArrowDownIcon,
            label: "Go to bottom",
            onClick: scrollToBottom,
          }
        : scrollTarget === "top" && !isAtTop
          ? {
              icon: ArrowUpIcon,
              label: "Go to top",
              onClick: scrollToTop,
            }
          : null;
  const ScrollChipIcon = scrollChip?.icon;
  const isBottomScrollChip = scrollTarget === "bottom";

  const handleSelectionAction = (action: string, selectedText: string) => {
    if (action === "copy") {
      void navigator.clipboard.writeText(selectedText);
    }
  };

  return (
    <div className="relative h-full">
      <div
        ref={handleContainerRef}
        data-transcript-container
        className={cn([
          "flex h-full flex-col gap-8 overflow-x-hidden overflow-y-auto",
          "scrollbar-hide",
          "scroll-pb-[calc(8rem+env(safe-area-inset-bottom))]",
          "pb-[calc(4rem+env(safe-area-inset-bottom))]",
        ])}
      >
        {visibleTranscriptIds.map((transcriptId, index) => (
          <div key={transcriptId} className="flex flex-col gap-8">
            <RenderTranscript
              scrollElement={scrollElement}
              isLastTranscript={index === visibleTranscriptIds.length - 1}
              shouldScrollToEnd={shouldScrollLastTranscriptToEnd}
              transcriptId={transcriptId}
              liveSegments={
                index === visibleTranscriptIds.length - 1 && currentActive
                  ? liveSegments
                  : []
              }
              currentMs={deferredCurrentMs}
              seek={seek}
              startPlayback={start}
              audioExists={audioExists}
            />
            {index < visibleTranscriptIds.length - 1 && <TranscriptSeparator />}
          </div>
        ))}

        <SelectionMenu
          containerRef={containerRef}
          onAction={handleSelectionAction}
        />
      </div>

      {scrollChip && (
        <button
          data-transcript-scroll-chip
          onClick={scrollChip.onClick}
          style={{
            [isBottomScrollChip ? "bottom" : "top"]: isBottomScrollChip
              ? "var(--transcript-scroll-chip-bottom, calc(1.5rem + env(safe-area-inset-bottom)))"
              : "var(--transcript-scroll-chip-top, calc(1.5rem + env(safe-area-inset-top)))",
          }}
          className={cn([
            "absolute left-1/2 z-30 inline-flex -translate-x-1/2 items-center gap-1.5",
            "border-border bg-muted text-foreground rounded-full border px-3 py-1.5",
            "hover:bg-muted active:bg-muted",
            "text-xs font-light",
            "transition-[top,bottom,background-color,border-color] duration-150",
          ])}
        >
          {ScrollChipIcon && (
            <ScrollChipIcon
              aria-hidden="true"
              className="size-3"
              strokeWidth={2.25}
            />
          )}
          {scrollChip.label}
        </button>
      )}
    </div>
  );
}
