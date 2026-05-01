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
  enablePlaybackControls = true,
}: {
  transcriptIds: string[];
  liveSegments: Segment[];
  currentActive: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
  enablePlaybackControls?: boolean;
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

  const { isAtBottom, autoScrollEnabled, scrollToBottom } =
    useScrollDetection(containerRef);

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
  const transcriptEntries =
    transcriptIds.length > 0
      ? transcriptIds.map((transcriptId) => ({ transcriptId }))
      : liveSegments.length > 0
        ? [{ transcriptId: undefined }]
        : [];

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
    {
      enabled: enablePlaybackControls,
      enableOnFormTags: false,
    },
  );

  usePlaybackAutoScroll(
    containerRef,
    deferredCurrentMs,
    enablePlaybackControls && isPlaying,
  );
  const shouldAutoScroll = currentActive && autoScrollEnabled;
  useAutoScroll(
    containerRef,
    [transcriptIds, liveSegments, shouldAutoScroll],
    shouldAutoScroll,
  );

  const shouldShowButton = !isAtBottom && currentActive;

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
          enablePlaybackControls ? "scroll-pb-32 pb-16" : "scroll-pb-4 pb-0",
        ])}
      >
        {transcriptEntries.map(({ transcriptId }, index) => (
          <div
            key={transcriptId ?? "live-transcript"}
            className="flex flex-col gap-8"
          >
            <RenderTranscript
              scrollElement={scrollElement}
              isLastTranscript={index === transcriptEntries.length - 1}
              isAtBottom={isAtBottom}
              transcriptId={transcriptId}
              liveSegments={
                index === transcriptEntries.length - 1 && currentActive
                  ? liveSegments
                  : []
              }
              currentMs={deferredCurrentMs}
              seek={seek}
              startPlayback={start}
              audioExists={audioExists}
            />
            {index < transcriptEntries.length - 1 && <TranscriptSeparator />}
          </div>
        ))}

        <SelectionMenu
          containerRef={containerRef}
          onAction={handleSelectionAction}
        />
      </div>

      <button
        onClick={scrollToBottom}
        className={cn([
          "absolute bottom-3 left-1/2 z-30 -translate-x-1/2",
          "rounded-full px-4 py-2",
          "bg-linear-to-t from-neutral-200 to-neutral-100 text-neutral-900",
          "shadow-xs hover:scale-[102%] hover:shadow-md active:scale-[98%]",
          "text-xs font-light",
          "transition-opacity duration-150",
          shouldShowButton ? "opacity-100" : "pointer-events-none opacity-0",
        ])}
      >
        Go to bottom
      </button>
    </div>
  );
}
