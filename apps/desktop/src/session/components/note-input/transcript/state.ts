import { useMemo } from "react";

import type { DegradedError } from "@hypr/plugin-listener";
import type { PartialWord, RuntimeSpeakerHint } from "@hypr/transcript";
import type { Operations } from "@hypr/transcript";

import { useAudioPlayer } from "~/audio-player";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";
import { parseTranscriptWords } from "~/stt/utils";

type ListeningStatus = "listening" | "finalizing";
type BatchPhase = "importing" | "transcribing";
type RecordingMode = "memory" | "disk" | null;
type RequestedTranscriptionMode = "live" | "batch" | null;

export type TranscriptScreen =
  | {
      kind: "running_batch";
      percentage?: number;
      phase?: BatchPhase;
    }
  | {
      kind: "batch_fallback";
      requestedTranscriptionMode: RequestedTranscriptionMode;
      error: DegradedError | null;
      recordingMode: RecordingMode;
    }
  | {
      kind: "listening";
      status: ListeningStatus;
    }
  | {
      kind: "empty";
      hasAudio: boolean;
      error: string | null;
    }
  | {
      kind: "ready";
      transcriptIds: string[];
      partialWords: PartialWord[];
      partialHints: RuntimeSpeakerHint[];
      editable: boolean;
      currentActive: boolean;
      operations?: Operations;
    };

export function useTranscriptScreen({
  sessionId,
  operations,
}: {
  sessionId: string;
  operations?: Operations;
}): TranscriptScreen {
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener(
    (state) => state.batch[sessionId]?.error ?? null,
  );
  const batchProgress = useListener((state) => state.batch[sessionId] ?? null);
  const live = useListener((state) => state.live);
  const { audioExists } = useAudioPlayer();

  const { transcriptIds, partialWords, partialHints, hasTranscriptWords } =
    useTranscriptContent(sessionId);

  const currentActive =
    sessionMode === "active" || sessionMode === "finalizing";
  const editable =
    sessionMode === "inactive" && Object.keys(operations ?? {}).length > 0;
  const isBatchMode =
    currentActive && live.currentTranscriptionMode === "batch";
  const hasVisibleTranscriptState =
    hasTranscriptWords || partialWords.length > 0 || !!batchError;

  if (sessionMode === "running_batch") {
    return {
      kind: "running_batch",
      percentage: batchProgress?.percentage,
      phase: batchProgress?.phase,
    };
  }

  if (isBatchMode) {
    return {
      kind: "batch_fallback",
      requestedTranscriptionMode: live.requestedTranscriptionMode,
      error: live.degraded,
      recordingMode: live.recordingMode,
    };
  }

  if (currentActive && !hasVisibleTranscriptState) {
    return {
      kind: "listening",
      status: sessionMode === "finalizing" ? "finalizing" : "listening",
    };
  }

  if (!hasVisibleTranscriptState) {
    return {
      kind: "empty",
      hasAudio: audioExists,
      error: batchError,
    };
  }

  return {
    kind: "ready",
    transcriptIds,
    partialWords,
    partialHints,
    editable,
    currentActive,
    operations,
  };
}

function useTranscriptContent(sessionId: string) {
  const transcriptIds =
    main.UI.useSliceRowIds(
      main.INDEXES.transcriptBySession,
      sessionId,
      main.STORE_ID,
    ) ?? [];
  const transcriptsTable = main.UI.useTable("transcripts", main.STORE_ID);
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

  const hasTranscriptWords = useMemo(() => {
    if (!store) {
      return false;
    }

    return transcriptIds.some(
      (transcriptId) => parseTranscriptWords(store, transcriptId).length > 0,
    );
  }, [store, transcriptIds, transcriptsTable]);

  return {
    transcriptIds,
    partialWords,
    partialHints,
    hasTranscriptWords,
  };
}
