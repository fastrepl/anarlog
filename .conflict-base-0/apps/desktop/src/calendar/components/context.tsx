import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
} from "react";
import {
  useManager,
  useRunningTaskRunIds,
  useScheduledTaskRunIds,
} from "tinytick/ui-react";

import {
  type CalendarSyncRange,
  CALENDAR_SYNC_TASK_ID,
  scheduleCalendarSync,
  syncCalendarEventsForRange,
} from "~/services/calendar";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

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
  const manager = useManager();
  const scheduledTaskRunIds = useScheduledTaskRunIds() ?? [];
  const runningTaskRunIds = useRunningTaskRunIds() ?? [];
  const toggleSyncTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(
    null,
  );
  const [isDebouncing, setIsDebouncing] = useState(false);
  const [rangeSyncCount, setRangeSyncCount] = useState(0);

  const isCalendarTaskRun = (taskRunId: string) =>
    manager?.getTaskRunInfo(taskRunId)?.taskId === CALENDAR_SYNC_TASK_ID;
  const isScheduled = scheduledTaskRunIds.some(isCalendarTaskRun);
  const isSyncing = runningTaskRunIds.some(isCalendarTaskRun);
  const isRangeSyncing = rangeSyncCount > 0;
  const canSync = manager !== undefined;

  const status: SyncStatus =
    isSyncing || isRangeSyncing
      ? "syncing"
      : isDebouncing || isScheduled
        ? "scheduled"
        : "idle";

  const scheduleSync = useCallback(() => {
    if (manager) {
      scheduleCalendarSync(manager);
    }
  }, [manager]);

  useMountEffect(() => {
    return () => {
      if (toggleSyncTimeoutRef.current) {
        clearTimeout(toggleSyncTimeoutRef.current);
        if (manager) {
          scheduleCalendarSync(manager);
        }
      }
    };
  });

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
