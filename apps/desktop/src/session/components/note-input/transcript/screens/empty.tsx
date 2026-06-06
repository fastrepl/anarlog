import { AlertCircleIcon, AudioLinesIcon } from "lucide-react";

import { Button } from "@hypr/ui/components/ui/button";
import { Spinner } from "@hypr/ui/components/ui/spinner";

export function TranscriptEmptyState({
  isBatching,
  hasAudio,
  percentage,
  phase,
  error,
  onUploadAudio,
  onUploadTranscript,
}: {
  isBatching?: boolean;
  hasAudio?: boolean;
  percentage?: number;
  phase?: "importing" | "transcribing";
  error?: string | null;
  onUploadAudio?: () => void;
  onUploadTranscript?: () => void;
}) {
  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <AlertCircleIcon className="h-8 w-8 text-red-400" />
        <div className="flex max-w-md flex-col gap-1">
          <p className="text-muted-foreground text-sm font-medium">
            Batch transcription failed
          </p>
          <p className="text-muted-foreground text-xs">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="text-muted-foreground flex h-full flex-col items-center justify-center gap-3">
      {isBatching ? (
        <Spinner size={28} />
      ) : (
        <AudioLinesIcon className="h-8 w-8" />
      )}
      {isBatching ? (
        <div className="flex flex-col items-center gap-1">
          {typeof percentage === "number" && percentage > 0 ? (
            <p className="text-muted-foreground text-2xl font-medium tabular-nums">
              {Math.round(percentage * 100)}%
            </p>
          ) : null}
          <p className="text-sm">
            {phase === "importing"
              ? "Importing audio..."
              : "Generating transcript..."}
          </p>
        </div>
      ) : (
        <div className="flex max-w-sm flex-col items-center gap-1 text-center">
          <p className="text-muted-foreground text-sm">
            {hasAudio ? "Recording available" : "No transcript available"}
          </p>
          <p className="text-muted-foreground text-xs">
            {hasAudio
              ? "Use the refresh button above to generate a transcript, or upload a file."
              : "Upload audio or a transcript file to populate this note."}
          </p>
          {(onUploadAudio || onUploadTranscript) && (
            <div className="mt-3 flex items-center gap-2">
              {onUploadAudio && (
                <Button variant="outline" size="sm" onClick={onUploadAudio}>
                  Upload audio
                </Button>
              )}
              {onUploadTranscript && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onUploadTranscript}
                >
                  Upload transcript
                </Button>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
