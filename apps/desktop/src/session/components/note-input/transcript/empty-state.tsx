import { AlertCircleIcon, AudioLinesIcon } from "lucide-react";

import { Spinner } from "@hypr/ui/components/ui/spinner";

export function TranscriptEmptyState({
  isBatching,
  percentage,
  phase,
  error,
}: {
  isBatching?: boolean;
  percentage?: number;
  phase?: "importing" | "transcribing";
  error?: string | null;
}) {
  if (error) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <AlertCircleIcon className="h-8 w-8 text-red-400" />
        <div className="flex max-w-md flex-col gap-1">
          <p className="text-sm font-medium text-neutral-700">
            Batch transcription failed
          </p>
          <p className="text-xs text-neutral-500">{error}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-neutral-400">
      {isBatching ? (
        <Spinner size={28} />
      ) : (
        <AudioLinesIcon className="h-8 w-8" />
      )}
      {isBatching ? (
        <div className="flex flex-col items-center gap-1">
          {typeof percentage === "number" && percentage > 0 ? (
            <p className="text-2xl font-medium text-neutral-500 tabular-nums">
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
        <p className="text-sm">No transcript available</p>
      )}
    </div>
  );
}
