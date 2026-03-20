import { type RefObject, useEffect, useRef } from "react";

import { useTranscriptOperations } from "./mutations";
import { TranscriptViewer } from "./renderer";
import { BatchState } from "./screens/batch";
import { TranscriptEmptyState } from "./screens/empty";
import { TranscriptListeningState } from "./screens/listening";
import { useTranscriptScreen } from "./state";

import { consumePendingUpload } from "~/stt/pending-upload";
import { useUploadFile } from "~/stt/useUploadFile";

export function Transcript({
  sessionId,
  isEditing,
  scrollRef,
}: {
  sessionId: string;
  isEditing: boolean;
  scrollRef: RefObject<HTMLDivElement | null>;
}) {
  const operations = useTranscriptOperations({ sessionId, isEditing });
  const screen = useTranscriptScreen({ sessionId, operations });
  const { uploadAudio, uploadTranscript, processFile } =
    useUploadFile(sessionId);

  const processFileRef = useRef(processFile);
  processFileRef.current = processFile;
  useEffect(() => {
    const pending = consumePendingUpload(sessionId);
    if (pending) {
      processFileRef.current(pending.filePath, pending.kind);
    }
  }, [sessionId]);

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
          partialWords={screen.partialWords}
          partialHints={screen.partialHints}
          editable={screen.editable}
          currentActive={screen.currentActive}
          operations={screen.operations}
          scrollRef={scrollRef}
        />
      )}
    </div>
  );
}
