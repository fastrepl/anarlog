import type { RefObject } from "react";

import { TranscriptViewer } from "./renderer";
import { BatchState } from "./screens/batch";
import { TranscriptEmptyState } from "./screens/empty";
import { TranscriptListeningState } from "./screens/listening";
import { useTranscriptScreen } from "./state";

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

  return (
    <div className="relative flex h-full flex-col overflow-hidden">
      {screen.kind === "running_batch" && (
        <TranscriptEmptyState
          isBatching
          percentage={screen.percentage}
          phase={screen.phase}
        />
      )}
      {screen.kind === "batch_fallback" && (
        <BatchState
          requestedTranscriptionMode={screen.requestedTranscriptionMode}
          error={screen.error}
          recordingMode={screen.recordingMode}
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
