import { DancingSticks } from "@anlg/ui/components/ui/dancing-sticks";

import { LiveTranscriptInterruptedNotice } from "./interrupted";

import { useListener } from "~/stt/contexts";

export function BatchState({
  requestedLiveTranscription,
}: {
  requestedLiveTranscription: boolean | null;
}) {
  const amplitude = useListener((state) => state.live.amplitude);

  if (requestedLiveTranscription === true) {
    return (
      <div className="flex h-full min-h-[400px] items-center justify-center">
        <LiveTranscriptInterruptedNotice />
      </div>
    );
  }

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
        <p className="text-base font-medium">Batch transcription mode</p>
        <p className="text-muted-foreground text-sm leading-relaxed">
          Recording continues. Your transcript will be generated after you stop.
        </p>
      </div>
    </div>
  );
}
