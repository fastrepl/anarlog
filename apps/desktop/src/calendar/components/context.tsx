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

import { CALENDAR_SYNC_TASK_ID } from "~/services/calendar";

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

  const isTaskRunning = useTaskRunRunning(pendingTaskRunId ?? "");
  const isSyncing = pendingTaskRunId !== null && isTaskRunning === true;

  const status: SyncStatus = isSyncing
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
