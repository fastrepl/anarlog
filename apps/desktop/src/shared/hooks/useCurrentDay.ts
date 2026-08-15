import { useMemo, useSyncExternalStore } from "react";

export function useCurrentDay(timezone?: string) {
  const store = useMemo(() => getCurrentDayStore(timezone), [timezone]);
  return useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
}

const currentDayStores = new Map<
  string,
  ReturnType<typeof createCurrentDayStore>
>();

function getCurrentDayStore(timezone: string | undefined) {
  const key = timezone ?? "";
  let store = currentDayStores.get(key);
  if (!store) {
    store = createCurrentDayStore(timezone);
    currentDayStores.set(key, store);
  }
  return store;
}

function createCurrentDayStore(timezone?: string) {
  const formatter = new Intl.DateTimeFormat("en-CA", {
    day: "2-digit",
    month: "2-digit",
    timeZone: timezone,
    year: "numeric",
  });
  const getCurrentDay = (timestamp = Date.now()) =>
    formatter.format(new Date(timestamp));
  let currentDay = getCurrentDay();
  let timeout: ReturnType<typeof setTimeout> | undefined;
  const listeners = new Set<() => void>();

  const getNextDayDelay = () => {
    const now = Date.now();
    const today = getCurrentDay(now);
    let low = now + 1;
    let high = now + 36 * 60 * 60 * 1000;
    while (getCurrentDay(high) === today) high += 12 * 60 * 60 * 1000;

    while (low < high) {
      const middle = Math.floor((low + high) / 2);
      if (getCurrentDay(middle) === today) low = middle + 1;
      else high = middle;
    }
    return Math.max(1, low - now);
  };

  const scheduleRefresh = () => {
    clearTimeout(timeout);
    timeout = undefined;
    if (listeners.size === 0 || document.visibilityState === "hidden") return;
    timeout = setTimeout(refresh, getNextDayDelay());
  };

  const refresh = () => {
    const nextDay = getCurrentDay();
    if (nextDay !== currentDay) {
      currentDay = nextDay;
      listeners.forEach((listener) => listener());
    }
    scheduleRefresh();
  };
  const handleVisibilityChange = () => {
    if (document.visibilityState === "visible") refresh();
    else scheduleRefresh();
  };

  const subscribe = (listener: () => void) => {
    if (listeners.size === 0) {
      window.addEventListener("focus", refresh);
      document.addEventListener("visibilitychange", handleVisibilityChange);
    }
    listeners.add(listener);
    refresh();

    return () => {
      listeners.delete(listener);
      if (listeners.size === 0) {
        scheduleRefresh();
        window.removeEventListener("focus", refresh);
        document.removeEventListener(
          "visibilitychange",
          handleVisibilityChange,
        );
      }
    };
  };

  return {
    getSnapshot: () => currentDay,
    subscribe,
  };
}
