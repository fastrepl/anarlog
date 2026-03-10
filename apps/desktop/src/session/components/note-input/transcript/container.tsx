import { type RefObject, useMemo } from "react";

import { type RuntimeSpeakerHint } from "@hypr/transcript";
import { cn } from "@hypr/utils";

import { BatchState } from "./batch-state";
import { TranscriptEmptyState } from "./empty-state";
import { LiveState } from "./live-state";
import { Operations } from "./operations";

import { useAudioPlayer } from "~/audio-player";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";
import { parseTranscriptWords } from "~/stt/utils";

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
  const live = useListener((state) => state.live);
  const degraded = live.degraded;
  const currentActive =
    sessionMode === "active" || sessionMode === "finalizing";
  const editable =
    sessionMode === "inactive" && Object.keys(operations ?? {}).length > 0;
  const requestedTranscriptionMode = live.requestedTranscriptionMode;
  const currentTranscriptionMode = live.currentTranscriptionMode;
  const recordingMode = live.recordingMode;
  const { audioExists } = useAudioPlayer();

  const partialWordsByChannel = useListener(
    (state) => state.partialWordsByChannel,
  );
  const partialHintsByChannel = useListener(
    (state) => state.partialHintsByChannel,
  );
  const store = main.UI.useStore(main.STORE_ID);

  const partialWords = useMemo(
    () => Object.values(partialWordsByChannel).flat(),
    [partialWordsByChannel],
  );
  const hasPersistedWords = useMemo(() => {
    if (!store) {
      return false;
    }

    return transcriptIds.some(
      (transcriptId) => parseTranscriptWords(store, transcriptId).length > 0,
    );
  }, [store, transcriptIds]);
  const hasTranscriptContent =
    hasPersistedWords || partialWords.length > 0 || !!batchError;
  const isBatchMode = currentActive && currentTranscriptionMode === "batch";

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

  if (sessionMode === "running_batch") {
    return (
      <div className="relative h-full">
        <div
          ref={(node) => {
            scrollRef.current = node;
          }}
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

  if (isBatchMode) {
    return (
      <BatchState
        requestedTranscriptionMode={requestedTranscriptionMode}
        error={degraded}
        recordingMode={recordingMode}
      />
    );
  }

  if (!hasTranscriptContent) {
    return (
      <TranscriptEmptyState
        isBatching={false}
        hasAudio={audioExists}
        error={batchError}
      />
    );
  }

  return (
    <LiveState
      transcriptIds={transcriptIds}
      partialWords={partialWords}
      partialHints={partialHints}
      editable={editable}
      currentActive={currentActive}
      operations={operations}
      scrollRef={scrollRef}
    />
  );
}
