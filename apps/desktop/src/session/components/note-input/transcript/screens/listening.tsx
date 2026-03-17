import { AudioLinesIcon } from "lucide-react";

import { Spinner } from "@hypr/ui/components/ui/spinner";

export function TranscriptListeningState({
  status,
}: {
  status: "listening" | "finalizing";
}) {
  const isFinalizing = status === "finalizing";

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 text-neutral-400">
      {isFinalizing ? (
        <Spinner size={28} />
      ) : (
        <AudioLinesIcon className="h-8 w-8" />
      )}
      <div className="flex max-w-sm flex-col items-center gap-1 text-center">
        <p className="text-sm text-neutral-500">
          {isFinalizing ? "Finalizing transcript..." : "Listening..."}
        </p>
        <p className="text-xs text-neutral-400">
          {isFinalizing
            ? "Transcript is still being written."
            : "Transcript will appear here when the first segment arrives."}
        </p>
      </div>
    </div>
  );
}
