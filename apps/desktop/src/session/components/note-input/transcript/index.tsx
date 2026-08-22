import { Trans } from "@lingui/react/macro";
import { CheckCircle, PencilSimple } from "@phosphor-icons/react";
import type { RefObject } from "react";
import { useCallback } from "react";

import { cn } from "@anlg/utils";

import { useRegenerateTranscript } from "./actions";
import { TranscriptViewer } from "./renderer";
import { BatchState } from "./screens/batch";
import { TranscriptEmptyState } from "./screens/empty";
import { TranscriptListeningState } from "./screens/listening";
import { useTranscriptScreen } from "./state";

import { useListener } from "~/stt/contexts";
import { useUploadFile } from "~/stt/useUploadFile";

export function TranscriptEditButton({
  editMode,
  onEditModeChange,
}: {
  editMode: boolean;
  onEditModeChange: (editMode: boolean) => void;
}) {
  return (
    <div className="mr-1 shrink-0">
      <button
        type="button"
        data-tauri-drag-region="false"
        aria-pressed={editMode}
        onClick={() => onEditModeChange(!editMode)}
        className={cn([
          "border-border bg-card text-foreground flex h-7 items-center gap-1.5 rounded-full border px-2 text-sm font-medium @max-[480px]:w-7 @max-[480px]:justify-center @max-[480px]:gap-0 @max-[480px]:px-0",
          "hover:bg-accent focus-visible:ring-ring transition-colors focus-visible:ring-2 focus-visible:outline-hidden",
          editMode ? "border-primary/30 bg-primary/10 text-primary" : null,
        ])}
      >
        {editMode ? (
          <CheckCircle aria-hidden className="size-3.5" />
        ) : (
          <PencilSimple aria-hidden className="size-3.5" />
        )}
        <span className="@max-[480px]:sr-only">
          {editMode ? <Trans>Done</Trans> : <Trans>Edit</Trans>}
        </span>
      </button>
    </div>
  );
}

export function Transcript({
  sessionId,
  scrollRef,
  editMode = false,
}: {
  sessionId: string;
  scrollRef: RefObject<HTMLDivElement | null>;
  editMode?: boolean;
}) {
  return (
    <TranscriptContent
      key={sessionId}
      sessionId={sessionId}
      scrollRef={scrollRef}
      editMode={editMode}
    />
  );
}

function TranscriptContent({
  sessionId,
  scrollRef,
  editMode,
}: {
  sessionId: string;
  scrollRef: RefObject<HTMLDivElement | null>;
  editMode: boolean;
}) {
  const screen = useTranscriptScreen({ sessionId });
  const { uploadAudio, uploadTranscript } = useUploadFile(sessionId);
  const regenerateTranscript = useRegenerateTranscript(sessionId);
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
          onRetranscribe={regenerateTranscript}
          onUploadAudio={uploadAudio}
          onUploadTranscript={uploadTranscript}
        />
      )}
      {screen.kind === "ready" && (
        <TranscriptViewer
          transcriptIds={screen.transcriptIds}
          liveSegments={screen.liveSegments}
          currentActive={screen.currentActive}
          captureGeneration={screen.captureGeneration}
          scrollRef={scrollRef}
          editMode={editMode && !screen.currentActive}
        />
      )}
    </div>
  );
}
