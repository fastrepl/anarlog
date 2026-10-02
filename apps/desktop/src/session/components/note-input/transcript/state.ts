import { useAudioPlayer } from "~/audio-player";
import {
  getLiveCaptureUiMode,
  isLiveTranscriptInterrupted,
} from "~/store/zustand/listener/general-shared";
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
      liveTranscriptInterrupted: boolean;
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
    liveTranscriptInterrupted,
    requestedLiveTranscription,
    sessionMode,
  } = useListener((state) => ({
    batchError: state.batch[sessionId]?.error ?? null,
    batchPercentage: state.batch[sessionId]?.percentage,
    batchPhase: state.batch[sessionId]?.phase,
    captureGeneration: state.live.captureGenerationBySession[sessionId] ?? 0,
    captureMode: getLiveCaptureUiMode(state.live),
    liveTranscriptInterrupted:
      state.live.sessionId === sessionId &&
      isLiveTranscriptInterrupted(state.live),
    requestedLiveTranscription: state.live.requestedLiveTranscription,
    sessionMode: state.getSessionMode(sessionId),
  }));
  const { audioExists } = useAudioPlayer();
  const currentActive =
    sessionMode === "active" || sessionMode === "finalizing";
  const { transcriptIds, liveSegments, hasTranscriptWords } =
    useTranscriptContent(sessionId, !currentActive);
  const isRecordOnlyMode =
    sessionMode === "active" && captureMode === "record_only";
  const hasVisibleTranscriptState =
    hasTranscriptWords || liveSegments.length > 0 || !!batchError;

  if (sessionMode === "running_batch") {
    return {
      kind: "running_batch",
      percentage: batchPercentage,
      phase: batchPhase,
    };
  }

  if (
    isRecordOnlyMode ||
    (liveTranscriptInterrupted && currentActive && !hasVisibleTranscriptState)
  ) {
    return {
      kind: "batch_fallback",
      requestedLiveTranscription,
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
    liveTranscriptInterrupted,
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
