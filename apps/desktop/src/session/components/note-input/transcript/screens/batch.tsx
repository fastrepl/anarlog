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
    <div className="flex h-full flex-col items-center justify-center gap-8">
      <DancingSticks
        amplitude={Math.min(Math.hypot(amplitude.mic, amplitude.speaker), 1)}
        color="#a3a3a3"
        height={56}
        width={120}
        stickWidth={3.5}
        gap={4}
      />
      <div className="flex max-w-sm flex-col items-center gap-2 text-center">
        <p className="text-base font-semibold text-neutral-700">
          {isFallbackFromLive
            ? "Live transcription stopped"
            : "Recording continues"}
        </p>
        <p className="text-sm leading-relaxed text-neutral-400">
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
