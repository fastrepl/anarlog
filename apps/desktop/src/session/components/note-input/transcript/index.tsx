import type { RefObject } from "react";
import { useCallback } from "react";

import { TranscriptViewer } from "./renderer";
import { BatchState } from "./screens/batch";
import { TranscriptEmptyState } from "./screens/empty";
import { TranscriptListeningState } from "./screens/listening";
import { useTranscriptScreen } from "./state";

import { useListener } from "~/stt/contexts";
import { useUploadFile } from "~/stt/useUploadFile";

export function Transcript({
  sessionId,
  scrollRef,
}: {
  sessionId: string;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const stopTranscription = useListener((state) => state.stopTranscription);
  const handleStopTranscription = useCallback(() => {
    void stopTranscription(sessionId);
  }, [sessionId, stopTranscription]);

  return (
    <div className="relative flex h-full flex-col overflow-hidden">
      {screen.kind === "running_batch" && (
        <TranscriptEmptyState
          isBatching
          percentage={screen.percentage}
          phase={screen.phase}
          onStopTranscription={
            screen.phase === "importing" ? undefined : handleStopTranscription
          }
        />
      )}
      {screen.kind === "batch_fallback" && (
        <BatchState
          requestedLiveTranscription={screen.requestedLiveTranscription}
          error={screen.error}
        />
      )}
      {screen.kind === "listening" && (
        <TranscriptListeningState status={screen.status} />
      )}
      {screen.kind === "empty" && (
        <TranscriptEmptyState
          isBatching={false}
          hasAudio={screen.hasAudio}
          error={screen.error}
          onUploadAudio={uploadAudio}
          onUploadTranscript={uploadTranscript}
        />
      )}
      {screen.kind === "ready" && (
        <TranscriptViewer
          transcriptIds={screen.transcriptIds}
          liveSegments={screen.liveSegments}
          currentActive={screen.currentActive}
          scrollRef={scrollRef}
        />
      )}
    </div>
  );
}
