import { t } from "@lingui/core/macro";

import {
  events as transcriptionEvents,
  type MeetingCaptureAutoStopReason,
} from "@anlg/plugin-transcription";
import { toast } from "@anlg/ui/components/ui/toast";
import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

function stopMessage(reason: MeetingCaptureAutoStopReason) {
  switch (reason) {
    case "appClosed":
      return t`The meeting app closed.`;
    case "durationLimit":
      return t`The recording reached the 12-hour limit.`;
    case "accountChanged":
      return t`You signed out or changed accounts.`;
  }
}

export function MeetingCaptureStopToasts() {
  useMountEffect(() => {
    let mounted = true;
    const subscription = transcriptionEvents.meetingCaptureAutoStopEvent
      .listen(({ payload }) => {
        if (!mounted) return;
        toast.info(t`Recording stopped`, {
          id: `meeting-capture-stop:${payload.capture_id}`,
          description: stopMessage(payload.reason),
        });
      })
      .catch((error) => {
        console.error(
          "[meeting-capture] failed to listen for automatic stops",
          error,
        );
      });
    return () => {
      mounted = false;
      void subscription.then((unlisten) => unlisten?.());
    };
  });

  return null;
}
