import { MessageSquareWarning } from "lucide-react";
import { useCallback } from "react";

import { cn } from "@hypr/utils";

import { FloatingButton } from "./floating/shared";

import { useListener } from "~/stt/contexts";
import {
  DISCLOSURE_VISIBLE_SECONDS,
  sendMeetingDisclosure,
} from "~/stt/disclosure";

type DisclosureVisibilityInput = {
  mode: "inactive" | "active" | "finalizing" | "running_batch";
  liveSessionId: string | null;
  sessionId: string;
  seconds: number;
  dismissed: boolean;
};

export function shouldShowMeetingDisclosureAction({
  mode,
  liveSessionId,
  sessionId,
  seconds,
  dismissed,
}: DisclosureVisibilityInput) {
  return (
    mode === "active" &&
    liveSessionId === sessionId &&
    seconds < DISCLOSURE_VISIBLE_SECONDS &&
    !dismissed
  );
}

function useMeetingDisclosureAction(sessionId: string) {
  const { visible, appIds, dismissMeetingDisclosure } = useListener((state) => {
    const dismissed = !!state.live.disclosureDismissedSessionIds[sessionId];

    return {
      visible: shouldShowMeetingDisclosureAction({
        mode: state.getSessionMode(sessionId),
        liveSessionId: state.live.sessionId,
        sessionId,
        seconds: state.live.seconds,
        dismissed,
      }),
      appIds: state.live.triggerAppIds,
      dismissMeetingDisclosure: state.dismissMeetingDisclosure,
    };
  });

  const handleClick = useCallback(() => {
    void sendMeetingDisclosure(appIds).then((disclosed) => {
      if (disclosed) {
        dismissMeetingDisclosure(sessionId);
      }
    });
  }, [appIds, dismissMeetingDisclosure, sessionId]);

  return { visible, handleClick };
}

export function FloatingDisclosureButton({ sessionId }: { sessionId: string }) {
  const { visible, handleClick } = useMeetingDisclosureAction(sessionId);

  if (!visible) {
    return null;
  }

  return (
    <FloatingButton
      onClick={handleClick}
      className={cn([
        "w-[126px] justify-start gap-2",
        "border-red-200 bg-red-50 pr-4 pl-3 text-red-600 shadow-[0_4px_14px_rgba(239,68,68,0.18)]",
        "hover:bg-red-100 hover:text-red-700",
      ])}
    >
      <MessageSquareWarning size={16} />
      <span>Disclose</span>
    </FloatingButton>
  );
}
