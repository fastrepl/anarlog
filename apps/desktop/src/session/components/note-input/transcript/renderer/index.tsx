import { type RefObject, useCallback, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { PartialWord, RuntimeSpeakerHint } from "@hypr/transcript";
import type { Operations } from "@hypr/transcript";
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

export function TranscriptViewer({
  transcriptIds,
  partialWords,
  partialHints,
  editable,
  currentActive,
  operations,
  scrollRef,
}: {
  transcriptIds: string[];
  partialWords: PartialWord[];
  partialHints: RuntimeSpeakerHint[];
  editable: boolean;
  currentActive: boolean;
  operations?: Operations;
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
  const currentMs = time.current * 1000;
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

  usePlaybackAutoScroll(containerRef, currentMs, isPlaying);
  const shouldAutoScroll = currentActive && autoScrollEnabled;
  useAutoScroll(
    containerRef,
    [transcriptIds, partialWords, shouldAutoScroll],
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
          "scrollbar-hide scroll-pb-32 pb-16",
        ])}
      >
        {transcriptIds.map((transcriptId, index) => (
          <div key={transcriptId} className="flex flex-col gap-8">
            <RenderTranscript
              scrollElement={scrollElement}
              isLastTranscript={index === transcriptIds.length - 1}
              isAtBottom={isAtBottom}
              editable={editable}
              transcriptId={transcriptId}
              partialWords={
                index === transcriptIds.length - 1 && currentActive
                  ? partialWords
                  : []
              }
              partialHints={
                index === transcriptIds.length - 1 && currentActive
                  ? partialHints
                  : []
              }
              operations={operations}
              currentMs={currentMs}
              seek={seek}
              startPlayback={start}
              audioExists={audioExists}
            />
            {index < transcriptIds.length - 1 && <TranscriptSeparator />}
          </div>
        ))}

        {editable && (
          <SelectionMenu
            containerRef={containerRef}
            onAction={handleSelectionAction}
          />
        )}
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
