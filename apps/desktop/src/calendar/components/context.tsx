import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";
import {
  useScheduleTaskRunCallback,
  useTaskRunRunning,
} from "tinytick/ui-react";

import {
  type CalendarSyncRange,
  CALENDAR_SYNC_TASK_ID,
  syncCalendarEventsForRange,
} from "~/services/calendar";

export const TOGGLE_SYNC_DEBOUNCE_MS = 5000;

export type SyncStatus = "idle" | "scheduled" | "syncing";

interface SyncContextValue {
  status: SyncStatus;
  canSync: boolean;
  scheduleSync: () => void;
  scheduleDebouncedSync: () => void;
  cancelDebouncedSync: () => void;
  syncRange: (range: CalendarSyncRange, signal?: AbortSignal) => Promise<void>;
}

const SyncContext = createContext<SyncContextValue | null>(null);

export function SyncProvider({ children }: { children: React.ReactNode }) {
  const scheduleEventSync = useScheduleTaskRunCallback(
    CALENDAR_SYNC_TASK_ID,
    undefined,
    0,
  );
  const toggleSyncTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const [pendingTaskRunId, setPendingTaskRunId] = useState<string | null>(null);
  const [isDebouncing, setIsDebouncing] = useState(false);
  const [rangeSyncCount, setRangeSyncCount] = useState(0);

  const isTaskRunning = useTaskRunRunning(pendingTaskRunId ?? "");
  const isSyncing = pendingTaskRunId !== null && isTaskRunning === true;
  const isRangeSyncing = rangeSyncCount > 0;
  const canSync = true;

  const status: SyncStatus =
    isSyncing || isRangeSyncing
      ? "syncing"
      : isDebouncing
        ? "scheduled"
        : "idle";

  useEffect(() => {
    if (pendingTaskRunId && isTaskRunning === false) {
      setPendingTaskRunId(null);
    }
  }, [pendingTaskRunId, isTaskRunning]);

  useEffect(() => {
    return () => {
      if (toggleSyncTimeoutRef.current) {
        clearTimeout(toggleSyncTimeoutRef.current);
        scheduleEventSync();
      }
    };
  }, [scheduleEventSync]);

  const scheduleSync = useCallback(() => {
    const taskRunId = scheduleEventSync();
    if (taskRunId) {
      setPendingTaskRunId(taskRunId);
    }
  }, [scheduleEventSync]);

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

  const syncRange = useCallback(
    async (range: CalendarSyncRange, signal?: AbortSignal) => {
      setRangeSyncCount((count) => count + 1);
      try {
        await syncCalendarEventsForRange(range, { signal });
      } finally {
        setRangeSyncCount((count) => Math.max(0, count - 1));
      }
    },
    [],
  );

  return (
    <SyncContext.Provider
      value={{
        status,
        canSync,
        scheduleSync,
        scheduleDebouncedSync,
        cancelDebouncedSync,
        syncRange,
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
