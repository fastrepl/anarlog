import { type RefObject, useCallback, useMemo, useRef, useState } from "react";
import { useHotkeys } from "react-hotkeys-hook";

import type { DegradedError } from "@hypr/plugin-listener";
import { type RuntimeSpeakerHint } from "@hypr/transcript";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";
import { cn } from "@hypr/utils";

import {
  useAutoScroll,
  usePlaybackAutoScroll,
  useScrollDetection,
} from "./hooks";
import { Operations } from "./operations";
import { RenderTranscript } from "./render-transcript";
import { SelectionMenu } from "./selection-menu";

import { useAudioPlayer } from "~/audio-player";
import { useAudioTime } from "~/audio-player/provider";
import { TranscriptEmptyState } from "~/session/components/note-input/transcript/empty-state";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";

export function TranscriptContainer({
  sessionId,
  operations,
  scrollRef,
}: {
  sessionId: string;
  operations?: Operations;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const transcriptIds = main.UI.useSliceRowIds(
    main.INDEXES.transcriptBySession,
    sessionId,
    main.STORE_ID,
  );

  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener(
    (state) => state.batch[sessionId]?.error ?? null,
  );
  const batchProgress = useListener((state) => state.batch[sessionId] ?? null);
  const degraded = useListener((state) => state.live.degraded);
  const currentActive =
    sessionMode === "active" || sessionMode === "finalizing";
  const editable =
    sessionMode === "inactive" && Object.keys(operations ?? {}).length > 0;

  const partialWordsByChannel = useListener(
    (state) => state.partialWordsByChannel,
  );
  const partialHintsByChannel = useListener(
    (state) => state.partialHintsByChannel,
  );

  const partialWords = useMemo(
    () => Object.values(partialWordsByChannel).flat(),
    [partialWordsByChannel],
  );

  const partialHints = useMemo(() => {
    const channelIndices = Object.keys(partialWordsByChannel)
      .map(Number)
      .sort((a, b) => a - b);

    const offsetByChannel = new Map<number, number>();
    let currentOffset = 0;
    for (const channelIndex of channelIndices) {
      offsetByChannel.set(channelIndex, currentOffset);
      currentOffset += partialWordsByChannel[channelIndex]?.length ?? 0;
    }

    const reindexedHints: RuntimeSpeakerHint[] = [];
    for (const channelIndex of channelIndices) {
      const hints = partialHintsByChannel[channelIndex] ?? [];
      const offset = offsetByChannel.get(channelIndex) ?? 0;
      for (const hint of hints) {
        reindexedHints.push({
          ...hint,
          wordIndex: hint.wordIndex + offset,
        });
      }
    }

    return reindexedHints;
  }, [partialWordsByChannel, partialHintsByChannel]);

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
    shouldAutoScroll || sessionMode === "running_batch",
  );

  const shouldShowButton = !isAtBottom && currentActive;

  if (sessionMode === "running_batch") {
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
          <TranscriptEmptyState
            isBatching
            percentage={batchProgress?.percentage}
            phase={batchProgress?.phase}
          />
        </div>
      </div>
    );
  }

  // TOOD: this can't handle words=[]
  if (transcriptIds.length === 0) {
    if (currentActive && degraded) {
      return <DegradedState error={degraded} />;
    }
    return <TranscriptEmptyState isBatching={false} error={batchError} />;
  }

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
        {currentActive && degraded ? (
          <DegradedState error={degraded} />
        ) : (
          transcriptIds.map((transcriptId, index) => (
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
          ))
        )}

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

function DegradedState({ error }: { error: DegradedError }) {
  const amplitude = useListener((state) => state.live.amplitude);

  return (
    <div className="flex h-full flex-col items-center justify-center gap-6">
      <DancingSticks
        amplitude={Math.min(Math.hypot(amplitude.mic, amplitude.speaker), 1)}
        color="#a3a3a3"
        height={40}
        width={80}
        stickWidth={3}
        gap={3}
      />
      <div className="flex flex-col items-center gap-1.5 text-center">
        <p className="text-sm font-medium text-neutral-600">
          Recording continues
        </p>
        <p className="text-xs text-neutral-400">{degradedMessage(error)}</p>
      </div>
    </div>
  );
}

function TranscriptSeparator() {
  return (
    <div
      className={cn([
        "flex items-center gap-3",
        "text-xs font-light text-neutral-400",
      ])}
    >
      <div className="flex-1 border-t border-neutral-200/40" />
      <span>~ ~ ~ ~ ~ ~ ~ ~ ~</span>
      <div className="flex-1 border-t border-neutral-200/40" />
    </div>
  );
}

function degradedMessage(error: DegradedError): string {
  switch (error.type) {
    case "authentication_failed":
      return `Authentication failed (${error.provider})`;
    case "upstream_unavailable":
      return error.message;
    case "connection_timeout":
      return "Transcription connection timed out";
    case "stream_error":
      return "Transcription stream error";
  }
}
