import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import {
  commands as calendarCommands,
  events as calendarEvents,
  type CalendarSyncEvent,
} from "@hypr/plugin-calendar";

export const TOGGLE_SYNC_DEBOUNCE_MS = 5000;

export type SyncStatus = "idle" | "scheduled" | "syncing";

interface SyncContextValue {
  status: SyncStatus;
  scheduleSync: () => void;
  scheduleDebouncedSync: () => void;
  cancelDebouncedSync: () => void;
}

const SyncContext = createContext<SyncContextValue | null>(null);

export function SyncProvider({ children }: { children: React.ReactNode }) {
  const toggleSyncTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const [workerStatus, setWorkerStatus] = useState<
    "idle" | "scheduled" | "running"
  >("idle");
  const [isDebouncing, setIsDebouncing] = useState(false);

  const refreshWorkerStatus = useCallback(async () => {
    try {
      const nextStatus = await calendarCommands.getCalendarSyncStatus();
      setWorkerStatus(nextStatus);
    } catch (error) {
      console.error(error);
      setWorkerStatus("idle");
    }
  }, []);

  const status: SyncStatus =
    workerStatus === "running"
      ? "syncing"
      : isDebouncing || workerStatus === "scheduled"
        ? "scheduled"
        : "idle";

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    let sawLiveEvent = false;

    const handleSyncEvent = ({ payload }: { payload: CalendarSyncEvent }) => {
      sawLiveEvent = true;
      switch (payload.type) {
        case "statusChanged":
          setWorkerStatus(payload.status);
          break;
        case "syncStarted":
          setWorkerStatus("running");
          break;
        case "syncFinished":
        case "syncFailed":
          setWorkerStatus("idle");
          break;
      }
    };

    void (async () => {
      try {
        const fn =
          await calendarEvents.calendarSyncEvent.listen(handleSyncEvent);
        if (cancelled) {
          fn();
          return;
        }
        unlisten = fn;
      } catch (error) {
        console.error(error);
      }

      try {
        const nextStatus = await calendarCommands.getCalendarSyncStatus();
        if (!cancelled && !sawLiveEvent) {
          setWorkerStatus(nextStatus);
        }
      } catch (error) {
        console.error(error);
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
      if (toggleSyncTimeoutRef.current) {
        clearTimeout(toggleSyncTimeoutRef.current);
        toggleSyncTimeoutRef.current = null;
        void calendarCommands
          .requestCalendarSync("manual")
          .catch(console.error);
      }
    };
  }, []);

  const scheduleSync = useCallback(() => {
    setWorkerStatus((current) => (current === "idle" ? "scheduled" : current));
    void calendarCommands
      .requestCalendarSync("manual")
      .then((accepted) => {
        if (!accepted) {
          void refreshWorkerStatus();
        }
      })
      .catch((error) => {
        console.error(error);
        void refreshWorkerStatus();
      });
  }, [refreshWorkerStatus]);

  const scheduleDebouncedSync = useCallback(() => {
    if (toggleSyncTimeoutRef.current) {
      clearTimeout(toggleSyncTimeoutRef.current);
    }
    setIsDebouncing(true);
    toggleSyncTimeoutRef.current = setTimeout(() => {
      toggleSyncTimeoutRef.current = null;
      setIsDebouncing(false);
      scheduleSync();
    }, TOGGLE_SYNC_DEBOUNCE_MS);
  }, [scheduleSync]);

  const cancelDebouncedSync = useCallback(() => {
    if (toggleSyncTimeoutRef.current) {
      clearTimeout(toggleSyncTimeoutRef.current);
      toggleSyncTimeoutRef.current = null;
      setIsDebouncing(false);
    }
  }, []);

  return (
    <SyncContext.Provider
      value={{
        status,
        scheduleSync,
        scheduleDebouncedSync,
        cancelDebouncedSync,
      }}
    >
      {children}
    </SyncContext.Provider>
  );
}

export function useSync() {
  const context = useContext(SyncContext);
  if (!context) {
    throw new Error("useSync must be used within a SyncProvider");
  }
  return context;
}
