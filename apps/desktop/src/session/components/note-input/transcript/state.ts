import { useMemo } from "react";

import type { DegradedError } from "@hypr/plugin-transcription";

import { useAudioPlayer } from "~/audio-player";
import * as main from "~/store/tinybase/store/main";
import { useListener } from "~/stt/contexts";
import type { Segment } from "~/stt/live-segment";
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
      liveSegments: Segment[];
      currentActive: boolean;
    };

export function useTranscriptScreen({
  sessionId,
}: {
  sessionId: string;
}): TranscriptScreen {
  const sessionMode = useListener((state) => state.getSessionMode(sessionId));
  const batchError = useListener(
    (state) => state.batch[sessionId]?.error ?? null,
  );
  const batchProgress = useListener((state) => state.batch[sessionId] ?? null);
  const live = useListener((state) => state.live);
  const { audioExists } = useAudioPlayer();

  const { transcriptIds, liveSegments, hasTranscriptWords } =
    useTranscriptContent(sessionId);

  const currentActive =
    sessionMode === "active" || sessionMode === "finalizing";
  const isBatchMode =
    currentActive && live.currentTranscriptionMode === "batch";
  const hasVisibleTranscriptState =
    hasTranscriptWords || liveSegments.length > 0 || !!batchError;

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
    liveSegments,
    currentActive,
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
  const liveSegments = useListener((state) => state.liveSegments);
  const store = main.UI.useStore(main.STORE_ID);

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
    liveSegments,
    hasTranscriptWords,
  };
}
