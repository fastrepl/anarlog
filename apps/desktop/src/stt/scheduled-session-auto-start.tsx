import { useRef } from "react";

import { isLockedFlag } from "~/lock/flag";
import { useAppLock } from "~/lock/store";
import { useSession } from "~/session/queries";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import { readDueScheduledSessionMeeting } from "~/stt/scheduled-auto-start";
import {
  beginScheduledAutoStart,
  finishScheduledAutoStart,
  isScheduledAutoStartInFlight,
} from "~/stt/scheduled-auto-start-state";
import { useStartListeningState } from "~/stt/useStartListening";

export function ScheduledSessionAutoStart({
  sessionId,
}: {
  sessionId: string;
}) {
  const canStartLiveSession = useListener((state) =>
    state.canStartLiveSession(sessionId),
  );
  const recordingActive = useListener(
    (state) => state.live.status === "active",
  );
  const session = useSession(sessionId);
  const revealed = useAppLock((state) =>
    Boolean(state.revealedNoteIds[sessionId]),
  );
  const locked = isLockedFlag(session?.locked) && !revealed;

  if (recordingActive || (session && locked)) {
    return <AbandonedScheduledSessionAutoStart sessionId={sessionId} />;
  }

  return canStartLiveSession && session ? (
    <ReadyScheduledSessionAutoStart sessionId={sessionId} />
  ) : (
    <PendingScheduledSessionAutoStart sessionId={sessionId} />
  );
}

function AbandonedScheduledSessionAutoStart({
  sessionId,
}: {
  sessionId: string;
}) {
  useMountEffect(() => {
    clearPendingAutoStart(sessionId);
  });

  return null;
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
  const { connectionReady, startListening } = useStartListeningState(
    sessionId,
    { automatic: true },
  );
  const attemptedRef = useRef(false);

  useMountEffect(() => {
    const timeout = setTimeout(() => clearPendingAutoStart(sessionId), 30_000);
    return () => clearTimeout(timeout);
  });

  return connectionReady ? (
    <StartScheduledSessionAutoStart
      attemptedRef={attemptedRef}
      sessionId={sessionId}
      startListening={startListening}
    />
  ) : null;
}

function StartScheduledSessionAutoStart({
  attemptedRef,
  sessionId,
  startListening,
}: {
  attemptedRef: { current: boolean };
  sessionId: string;
  startListening: () => Promise<void>;
}) {
  const startListeningRef = useLatestRef(startListening);

  useMountEffect(() => {
    if (attemptedRef.current) {
      return;
    }
    attemptedRef.current = true;
    let cancelled = false;

    void readDueScheduledSessionMeeting(sessionId)
      .then((meeting) => {
        if (cancelled) return;
        clearPendingAutoStart(sessionId);
        if (!meeting || isScheduledAutoStartInFlight(sessionId)) {
          return;
        }

        beginScheduledAutoStart(sessionId);
        return startListeningRef.current().finally(() => {
          finishScheduledAutoStart(sessionId);
        });
      })
      .catch((error) => {
        if (cancelled) return;
        clearPendingAutoStart(sessionId);
        console.error("[listener] failed to auto-start session", error);
      });

    return () => {
      cancelled = true;
    };
  });

  return null;
}

function clearPendingAutoStart(sessionId: string) {
  const tabsState = useTabs.getState();
  const currentTab = tabsState.tabs.find(
    (candidate): candidate is Extract<Tab, { type: "sessions" }> =>
      candidate.type === "sessions" && candidate.id === sessionId,
  );
  if (!currentTab?.state.autoStart) {
    return;
  }

  tabsState.updateSessionTabState(currentTab, {
    ...currentTab.state,
    autoStart: null,
  });
}
