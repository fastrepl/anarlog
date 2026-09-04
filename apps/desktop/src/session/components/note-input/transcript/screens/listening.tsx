import { Waveform } from "@anlg/ui/components/icons";
import { Spinner } from "@anlg/ui/components/ui/spinner";

export function TranscriptListeningState({
  status,
}: {
  status: "listening" | "finalizing";
}) {
  const isFinalizing = status === "finalizing";

  return (
    <div
      role="status"
      className="flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center"
    >
      {isFinalizing ? (
        <div className="text-muted-foreground mb-5">
          <Spinner size={36} />
        </div>
      ) : (
        <Waveform
          aria-hidden
          className="text-muted-foreground mb-5 size-9 stroke-[1.5]"
        />
      )}
      <div className="flex max-w-md flex-col gap-2">
        <p className="text-base font-medium">
          {isFinalizing ? "Finalizing transcript..." : "Listening..."}
        </p>
        <p className="text-muted-foreground text-sm leading-relaxed">
          {isFinalizing
            ? "Transcript is still being written."
            : "Transcript will appear here when the first segment arrives."}
        </p>
      </div>
    </div>
  );
}
