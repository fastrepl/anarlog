import { useCallback } from "react";

import { ListenActionButton } from "../listen-action";

import { useListenButtonState } from "~/session/components/shared";
import { useEventCountdown } from "~/session/hooks/useEventCountdown";
import { useConfigValue } from "~/shared/config";
import type { Tab } from "~/store/zustand/tabs";
import { useTabs } from "~/store/zustand/tabs";
import { useListener } from "~/stt/contexts";

export function ListenButton({
  tab,
}: {
  tab: Extract<Tab, { type: "sessions" }>;
}) {
  const { shouldRender } = useListenButtonState(tab.id);
  const loading = useListener(
    (state) => state.live.loading && state.live.sessionId === tab.id,
  );
  const canStartLiveSession = useListener((state) =>
    state.canStartLiveSession(tab.id),
  );
  const autoStartScheduledMeetings = useConfigValue(
    "auto_start_scheduled_meetings",
  );
  const updateSessionTabState = useTabs((state) => state.updateSessionTabState);
  const handleCountdownExpire = useCallback(() => {
    if (!autoStartScheduledMeetings || !canStartLiveSession) {
      return;
    }

    updateSessionTabState(tab, { ...tab.state, autoStart: true });
  }, [
    autoStartScheduledMeetings,
    canStartLiveSession,
    tab,
    updateSessionTabState,
  ]);
  const countdown = useEventCountdown(tab.id, {
    onExpire: handleCountdownExpire,
  });

  if (loading) {
    return <ListenActionButton sessionId={tab.id} />;
  }

  if (!shouldRender) {
    return null;
  }

  return (
    <div className="flex flex-col items-center gap-2">
      {countdown.label && (
        <div className="text-xs whitespace-nowrap text-neutral-500">
          <span>{countdown.label}</span>
        </div>
      )}
      <ListenActionButton sessionId={tab.id} />
    </div>
  );
}
