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
    isNearBottom,
    canScroll,
    autoScrollEnabled,
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
  const shouldScrollLastTranscriptToEnd = currentActive && isNearBottom;
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

      {canScroll && (
        <div
          data-transcript-scroll-controls
          className={cn([
            "absolute top-1/2 right-1 z-40 flex -translate-y-1/2 flex-col overflow-hidden",
            "border-border/60 bg-muted/70 text-foreground rounded-full border",
          ])}
        >
          <button
            type="button"
            aria-label="Scroll to top"
            onClick={scrollToTop}
            disabled={isAtTop}
            className={cn([
              "flex size-8 items-center justify-center",
              "hover:bg-muted/85 active:bg-muted/85",
              "disabled:pointer-events-none disabled:opacity-30",
            ])}
          >
            <ArrowUpIcon aria-hidden="true" className="size-3.5" />
          </button>
          <div className="bg-border/70 h-px w-full" />
          <button
            type="button"
            aria-label="Scroll to bottom"
            onClick={scrollToBottom}
            disabled={isAtBottom}
            className={cn([
              "flex size-8 items-center justify-center",
              "hover:bg-muted/85 active:bg-muted/85",
              "disabled:pointer-events-none disabled:opacity-30",
            ])}
          >
            <ArrowDownIcon aria-hidden="true" className="size-3.5" />
          </button>
        </div>
      )}
    </div>
  );
}
