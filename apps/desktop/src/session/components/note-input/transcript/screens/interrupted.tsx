import { cn } from "@anlg/utils";

import { useStoredSettingValue } from "~/settings/queries";
import { useListener } from "~/stt/contexts";
import { getLiveTranscriptPausedMessage } from "~/stt/live-transcript-interrupted";

export function LiveTranscriptInterruptedNotice({
  className,
}: {
  className?: string;
}) {
  const degraded = useListener((state) => state.live.degraded);
  const sttProvider = useStoredSettingValue("current_stt_provider").value;

  return (
    <p
      role="status"
      className={cn([
        "text-muted-foreground mx-auto max-w-md px-6 text-center text-sm leading-relaxed",
        className,
      ])}
    >
      {getLiveTranscriptPausedMessage({ degraded, sttProvider })}
    </p>
  );
}
