import { useRef } from "react";

import { isLockedFlag } from "~/lock/flag";
import { useAppLock } from "~/lock/store";
import { useSession } from "~/session/queries";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
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
  const session = useSession(sessionId);
  const revealed = useAppLock((state) =>
    Boolean(state.revealedNoteIds[sessionId]),
  );
  const locked = isLockedFlag(session?.locked) && !revealed;

  if (session && locked) {
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
  const { connectionReady, startListening } = useStartListeningState(sessionId);
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
    clearPendingAutoStart(sessionId);

    // Re-arming a session whose start is still in flight (a second trigger
    // before capture becomes active) must not start a second lifecycle: the
    // two would race for the same capture marker and one fails with a toast.
    if (isScheduledAutoStartInFlight(sessionId)) {
      return;
    }
    beginScheduledAutoStart(sessionId);

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
