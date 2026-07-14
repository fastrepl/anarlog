import type { DegradedError } from "@hypr/plugin-transcription";
import { DancingSticks } from "@hypr/ui/components/ui/dancing-sticks";

import { useListener } from "~/stt/contexts";

export function BatchState({
  requestedLiveTranscription,
  error,
}: {
  requestedLiveTranscription: boolean | null;
  error: DegradedError | null;
}) {
  const amplitude = useListener((state) => state.live.amplitude);
  const isFallbackFromLive = requestedLiveTranscription === true;
  const continuation =
    "Recording continues and audio will be saved when you stop.";

  return (
    <div
      role="status"
      className="flex h-full min-h-[400px] flex-col items-center justify-center px-6 text-center"
    >
      <div className="mb-5">
        <DancingSticks
          amplitude={Math.min(Math.hypot(amplitude.mic, amplitude.speaker), 1)}
          color="#a3a3a3"
          height={36}
          width={80}
          stickWidth={3}
          gap={3}
        />
      </div>
      <div className="flex max-w-md flex-col gap-2">
        <p className="text-base font-medium">
          {isFallbackFromLive
            ? "Live transcription stopped"
            : "Recording continues"}
        </p>
        <p className="text-muted-foreground text-sm leading-relaxed">
          {isFallbackFromLive
            ? `${error ? degradedMessage(error) : "Live transcription is unavailable."} ${continuation}`
            : `${continuation}`}
        </p>
      </div>
    </div>
  );
}

function degradedMessage(error: DegradedError): string {
  switch (error.type) {
    case "authentication_failed":
      return `Authentication failed (${error.provider})`;
    case "upstream_unavailable":
      return error.message;
    case "connection_timeout":
      return "Transcription connection timed out";
    case "stream_error":
      return "Transcription stream error";
  }
}
