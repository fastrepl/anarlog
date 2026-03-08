import { AlertCircleIcon } from "lucide-react";
import { useMemo } from "react";

import { Spinner } from "@hypr/ui/components/ui/spinner";

import { useListener } from "~/stt/contexts";

export function TranscriptionProgress({ sessionId }: { sessionId: string }) {
  const { progress: progressRaw, mode } = useListener((state) => ({
    progress: state.batch[sessionId] ?? null,
    mode: state.getSessionMode(sessionId),
  }));

  const isRunning = mode === "running_batch";
  const hasError = progressRaw?.error != null;

  const statusLabel = useMemo(() => {
    if (!progressRaw || progressRaw.percentage === 0) {
      if (progressRaw?.phase === "importing") {
        return "Importing audio...";
      }
      return "Processing...";
    }

    const percent = Math.round(progressRaw.percentage * 100);
    return `${percent}%`;
  }, [progressRaw]);

  if (hasError) {
    return (
      <div className="mb-3">
        <div className="flex w-fit items-center gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700 shadow-xs">
          <AlertCircleIcon size={14} className="shrink-0 text-red-500" />
          <div className="flex flex-col gap-0.5">
            <span className="font-medium">Transcription failed</span>
            <span className="text-red-600">{progressRaw.error}</span>
          </div>
        </div>
      </div>
    );
  }

  if (!isRunning) {
    return null;
  }

  return (
    <div className="mb-3">
      <div className="flex w-fit items-center gap-2 rounded-full bg-neutral-900 px-3 py-1 text-xs text-white shadow-xs">
        <Spinner size={12} className="text-white/80" />
        <span>
          {!progressRaw || progressRaw.percentage === 0
            ? statusLabel
            : `Processing · ${statusLabel}`}
        </span>
      </div>
    </div>
  );
}
