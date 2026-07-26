import { useMemo, useSyncExternalStore } from "react";

export function useCurrentDay(timezone?: string) {
  const store = useMemo(() => createCurrentDayStore(timezone), [timezone]);
  return useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
}

function createCurrentDayStore(timezone?: string) {
  const getCurrentDay = () =>
    new Intl.DateTimeFormat("en-CA", {
      day: "2-digit",
      month: "2-digit",
      timeZone: timezone,
      year: "numeric",
    }).format(new Date());
  let currentDay = getCurrentDay();

  const subscribe = (listener: () => void) => {
    const refresh = () => {
      const nextDay = getCurrentDay();
      if (nextDay === currentDay) return;
      currentDay = nextDay;
      listener();
    };
    const interval = setInterval(refresh, 60_000);
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") refresh();
    };

    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    refresh();

    return () => {
      clearInterval(interval);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  };

  return {
    getSnapshot: () => currentDay,
    subscribe,
  };
}
