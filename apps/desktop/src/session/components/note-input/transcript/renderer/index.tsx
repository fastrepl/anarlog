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
    autoScrollEnabled,
    scrollTarget,
    scrollToTop,
    scrollToBottom,
  } = useScrollDetection(containerRef);

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

  const scrollChip =
    currentActive && scrollTarget === "bottom" && !isAtBottom
      ? {
          label: "Go to bottom",
          onClick: scrollToBottom,
        }
      : currentActive && scrollTarget === "top" && !isAtTop
        ? {
            label: "Go to top",
            onClick: scrollToTop,
          }
        : null;

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
        {transcriptIds.map((transcriptId, index) => (
          <div key={transcriptId} className="flex flex-col gap-8">
            <RenderTranscript
              scrollElement={scrollElement}
              isLastTranscript={index === transcriptIds.length - 1}
              shouldScrollToEnd={shouldScrollLastTranscriptToEnd}
              transcriptId={transcriptId}
              liveSegments={
                index === transcriptIds.length - 1 && currentActive
                  ? liveSegments
                  : []
              }
              currentMs={deferredCurrentMs}
              seek={seek}
              startPlayback={start}
              audioExists={audioExists}
            />
            {index < transcriptIds.length - 1 && <TranscriptSeparator />}
          </div>
        ))}

        <SelectionMenu
          containerRef={containerRef}
          onAction={handleSelectionAction}
        />
      </div>

      {scrollChip && (
        <button
          onClick={scrollChip.onClick}
          className={cn([
            "absolute bottom-[calc(5rem+env(safe-area-inset-bottom))] left-1/2 z-30 -translate-x-1/2",
            "rounded-full px-4 py-2",
            "from-muted to-accent text-foreground bg-linear-to-t",
            "shadow-xs hover:scale-[102%] hover:shadow-md active:scale-[98%]",
            "text-xs font-light",
            "transition-opacity duration-150",
          ])}
        >
          {scrollChip.label}
        </button>
      )}
    </div>
  );
}
