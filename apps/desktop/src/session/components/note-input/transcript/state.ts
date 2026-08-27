import type { DegradedError } from "@anlg/plugin-transcription";

import { useAudioPlayer } from "~/audio-player";
import { getLiveCaptureUiMode } from "~/store/zustand/listener/general-shared";
import { useListener } from "~/stt/contexts";
import type { Segment } from "~/stt/live-segment";
import { useSessionTranscriptMetadata } from "~/stt/queries";

type ListeningStatus = "listening" | "finalizing";
type BatchPhase = "importing" | "transcribing";
type RequestedLiveTranscription = boolean | null;

export type TranscriptScreen =
  | {
      kind: "running_batch";
      percentage?: number;
      phase?: BatchPhase;
    }
  | {
      kind: "batch_fallback";
      requestedLiveTranscription: RequestedLiveTranscription;
      error: DegradedError | null;
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
      captureGeneration: number;
    };

export function useTranscriptScreen({
  sessionId,
}: {
  sessionId: string;
}): TranscriptScreen {
  const {
    batchError,
    batchPercentage,
    batchPhase,
    captureGeneration,
    captureMode,
    degraded,
    requestedLiveTranscription,
    sessionMode,
  } = useListener((state) => ({
    batchError: state.batch[sessionId]?.error ?? null,
    batchPercentage: state.batch[sessionId]?.percentage,
    batchPhase: state.batch[sessionId]?.phase,
    captureGeneration: state.live.captureGenerationBySession[sessionId] ?? 0,
    captureMode: getLiveCaptureUiMode(state.live),
    degraded: state.live.degraded,
    requestedLiveTranscription: state.live.requestedLiveTranscription,
    sessionMode: state.getSessionMode(sessionId),
  }));
  const { audioExists } = useAudioPlayer();
  const currentActive =
    sessionMode === "active" || sessionMode === "finalizing";
  const { transcriptIds, liveSegments, hasTranscriptWords } =
    useTranscriptContent(sessionId, !currentActive);
  const isRecordOnlyMode = sessionMode === "active" && captureMode !== "live";
  const hasVisibleTranscriptState =
    hasTranscriptWords || liveSegments.length > 0 || !!batchError;

  if (sessionMode === "running_batch") {
    return {
      kind: "running_batch",
      percentage: batchPercentage,
      phase: batchPhase,
    };
  }

  if (isRecordOnlyMode) {
    return {
      kind: "batch_fallback",
      requestedLiveTranscription,
      error: degraded,
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
    captureGeneration,
  };
}

function useTranscriptContent(
  sessionId: string,
  includePendingDeltas: boolean,
) {
  const transcripts = useSessionTranscriptMetadata(
    sessionId,
    includePendingDeltas,
  );
  const liveSegments = useListener((state) => state.liveSegments);

  return {
    transcriptIds: transcripts.map((transcript) => transcript.id),
    liveSegments,
    hasTranscriptWords: transcripts.some((transcript) => transcript.hasWords),
  };
}
