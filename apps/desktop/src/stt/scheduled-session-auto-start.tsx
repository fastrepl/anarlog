import { useRef } from "react";

import { useMountEffect } from "~/shared/hooks/useMountEffect";
import { type Tab, useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";
import {
  beginScheduledAutoStart,
  finishScheduledAutoStart,
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

  return canStartLiveSession ? (
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
  const attemptedRef = useRef(false);

  useMountEffect(() => {
    if (attemptedRef.current) {
      return;
    }
    attemptedRef.current = true;
    beginScheduledAutoStart(sessionId);
    clearPendingAutoStart(sessionId);

    void startListening()
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
