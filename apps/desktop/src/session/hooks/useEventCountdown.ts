import { useMemo, useSyncExternalStore } from "react";

import { useSessionEvent } from "~/session/hooks/useSessionEvent";

const FIVE_MINUTES = 5 * 60 * 1000;

export function useEventCountdown(sessionId: string) {
  const sessionEvent = useSessionEvent(sessionId);
  const startedAt = sessionEvent?.started_at;
  const store = useMemo(() => createCountdownStore(startedAt), [startedAt]);
  const label = useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );

  return { label };
}

function getCountdownLabel(startedAt: string | undefined, nowMs: number) {
  if (!startedAt) return null;
  const diff = new Date(startedAt).getTime() - nowMs;
  if (diff <= 0 || diff > FIVE_MINUTES) return null;

  const totalSeconds = Math.floor(diff / 1000);
  const mins = Math.floor(totalSeconds / 60);
  const secs = totalSeconds % 60;
  return mins > 0 ? `starts in ${mins}m ${secs}s` : `starts in ${secs}s`;
}

function createCountdownStore(startedAt: string | undefined) {
  const eventStart = startedAt ? new Date(startedAt).getTime() : Number.NaN;
  let label = getCountdownLabel(startedAt, Date.now());
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<() => void>();

  const scheduleRefresh = () => {
    clearTimeout(timeout);
    timeout = undefined;
    if (
      listeners.size === 0 ||
      !Number.isFinite(eventStart) ||
      document.visibilityState === "hidden"
    ) {
      return;
    }

    const diff = eventStart - Date.now();
    if (diff <= 0) return;
    if (diff > FIVE_MINUTES) {
      timeout = setTimeout(refresh, Math.ceil(diff - FIVE_MINUTES));
      return;
    }

    const remainder = diff % 1000;
    timeout = setTimeout(
      refresh,
      remainder === 0 ? 1000 : Math.max(1, Math.floor(remainder) + 1),
    );
  };

  const refresh = () => {
    const nextLabel = getCountdownLabel(startedAt, Date.now());
    if (nextLabel !== label) {
      label = nextLabel;
      listeners.forEach((listener) => listener());
    }
    scheduleRefresh();
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") refresh();
      else scheduleRefresh();
    };
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    refresh();

    return () => {
      listeners.delete(listener);
      scheduleRefresh();
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  };

  return { getSnapshot: () => label, subscribe };
}
