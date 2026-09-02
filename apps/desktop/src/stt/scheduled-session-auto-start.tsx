import { useRef } from "react";

import { commands as openerCommands } from "@anlg/plugin-opener2";

import { useSession } from "~/session/queries";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import {
  beginScheduledAutoStart,
  finishScheduledAutoStart,
  takeScheduledAutoJoin,
} from "~/stt/scheduled-auto-start-state";
import { useStartListening } from "~/stt/useStartListening";

export function ScheduledSessionAutoStart({
  sessionId,
}: {
  sessionId: string;
}) {
  const canStartLiveSession = useListener((state) =>
    state.canStartLiveSession(sessionId),
  );
  const session = useSession(sessionId);

  return canStartLiveSession && session ? (
    <ReadyScheduledSessionAutoStart sessionId={sessionId} />
  ) : (
    <PendingScheduledSessionAutoStart sessionId={sessionId} />
  );
}

function PendingScheduledSessionAutoStart({
  sessionId,
}: {
  sessionId: string;
}) {
  useMountEffect(() => {
    const timeout = setTimeout(() => clearPendingAutoStart(sessionId), 30_000);
    return () => clearTimeout(timeout);
  });

  return null;
}

function ReadyScheduledSessionAutoStart({ sessionId }: { sessionId: string }) {
  const startListening = useStartListening(sessionId);
  const startListeningRef = useLatestRef(startListening);
  const attemptedRef = useRef(false);

  useMountEffect(() => {
    if (attemptedRef.current) {
      return;
    }
    attemptedRef.current = true;
    beginScheduledAutoStart(sessionId);
    const meetingLink = clearPendingAutoStart(sessionId);

    // Match Join & record: start capture together with opening the meeting.
    // Opening the link first lets the meeting app take audio devices before we
    // listen, which is what the scheduled-time path was doing.
    if (meetingLink) {
      void openerCommands.openUrl(meetingLink, null);
    }

    void startListeningRef
      .current()
      .catch((error) => {
        console.error("[listener] failed to auto-start session", error);
      })
      .finally(() => {
        finishScheduledAutoStart(sessionId);
      });
  });

  return null;
}

function clearPendingAutoStart(sessionId: string) {
  const meetingLink = takeScheduledAutoJoin(sessionId);
  const tabsState = useTabs.getState();
  const currentTab = tabsState.tabs.find(
    (candidate): candidate is Extract<Tab, { type: "sessions" }> =>
      candidate.type === "sessions" && candidate.id === sessionId,
  );
  if (!currentTab?.state.autoStart) {
    return meetingLink;
  }

  tabsState.updateSessionTabState(currentTab, {
    ...currentTab.state,
    autoStart: null,
  });
  return meetingLink;
}
