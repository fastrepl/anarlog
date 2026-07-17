import { useLingui } from "@lingui/react/macro";
import { useCallback, useMemo, useSyncExternalStore } from "react";

import {
  buildTimelineBuckets,
  deriveTimelineWindowData,
  getItemTimeRange,
  type TimelineBucket,
} from "./utils";

import { useIgnoredEvents } from "~/calendar/ignored-events";
import { useTimelineTables } from "~/calendar/queries";
import { useConfigValue } from "~/shared/config";
import { useCurrentDay } from "~/shared/hooks/useCurrentDay";

const UPCOMING_MEETING_VISIBLE_WINDOW_MS = 5 * 60 * 1000;
const UPCOMING_MEETING_STATUS_TICK_MS = 1_000;

export type SidebarUpcomingMeetingStatus = {
  itemKey: string;
  label: string;
  title: string;
  progress?: number;
};

export function useSidebarUpcomingMeetingStatus({
  showIgnored = false,
}: {
  showIgnored?: boolean;
} = {}): SidebarUpcomingMeetingStatus | null {
  const timezone = useConfigValue("timezone") || undefined;
  const { t } = useLingui();
  const { isIgnored } = useIgnoredEvents();
  const formatLabel = useUpcomingMeetingLabelFormatter();
  const { timelineEventsTable, timelineSessionsTable } = useTimelineTables();
  const currentDay = useCurrentDay(timezone);

  const buckets = useMemo(() => {
    const windowData = deriveTimelineWindowData({
      isEventIgnored: isIgnored,
      showIgnored,
      timelineEventsTable,
      timelineSessionsTable,
      timezone,
    });

    return buildTimelineBuckets({
      timelineEventsTable: windowData.timelineEventsTable,
      timelineSessionsTable: windowData.timelineSessionsTable,
      timezone,
    });
  }, [
    isIgnored,
    currentDay,
    showIgnored,
    timelineEventsTable,
    timelineSessionsTable,
    timezone,
  ]);

  return useUpcomingMeetingStatus(buckets, formatLabel, t`Now`);
}

export function useUpcomingMeetingStatus(
  buckets: TimelineBucket[],
  formatLabel: (diffMs: number) => string,
  activeLabel: string,
): SidebarUpcomingMeetingStatus | null {
  const store = useMemo(
    () => createUpcomingMeetingStatusStore(buckets, formatLabel, activeLabel),
    [activeLabel, buckets, formatLabel],
  );

  return useSyncExternalStore(
    store.subscribe,
    store.getSnapshot,
    store.getSnapshot,
  );
}

function createUpcomingMeetingStatusStore(
  buckets: TimelineBucket[],
  formatLabel: (diffMs: number) => string,
  activeLabel: string,
) {
  const getCurrentStatus = () =>
    getUpcomingMeetingStatus(
      buckets,
      Math.floor(Date.now() / UPCOMING_MEETING_STATUS_TICK_MS) *
        UPCOMING_MEETING_STATUS_TICK_MS,
      formatLabel,
      activeLabel,
    );
  let status = getCurrentStatus();
  const listeners = new Set<() => void>();

  const refresh = () => {
    const nextStatus = getCurrentStatus();
    if (upcomingMeetingStatusesAreEqual(status, nextStatus)) {
      return;
    }

    status = nextStatus;
    listeners.forEach((listener) => listener());
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    const interval = setInterval(refresh, UPCOMING_MEETING_STATUS_TICK_MS);
    const handleVisibilityChange = () => {
      if (document.visibilityState === "visible") {
        refresh();
      }
    };

    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", handleVisibilityChange);
    refresh();

    return () => {
      listeners.delete(listener);
      clearInterval(interval);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  };

  return {
    getSnapshot: () => status,
    subscribe,
  };
}

export function getUpcomingMeetingStatus(
  buckets: TimelineBucket[],
  currentTimeMs: number,
  formatLabel: (diffMs: number) => string = formatUpcomingMeetingLabelEnglish,
  activeLabel = "Now",
): SidebarUpcomingMeetingStatus | null {
  let active: { itemKey: string; title: string; endsAtMs: number } | null =
    null;
  let nearest: { itemKey: string; title: string; diffMs: number } | null = null;

  for (const bucket of buckets) {
    for (const item of bucket.items) {
      if (item.type === "event" && item.data.is_all_day) {
        continue;
      }

      const { start, end } = getItemTimeRange(item);
      if (!start) {
        continue;
      }

      const startsAtMs = start.getTime();
      const endsAtMs = end?.getTime();
      if (
        typeof endsAtMs === "number" &&
        endsAtMs > startsAtMs &&
        startsAtMs <= currentTimeMs &&
        currentTimeMs <= endsAtMs
      ) {
        if (!active || endsAtMs < active.endsAtMs) {
          active = {
            itemKey: `${item.type}-${item.id}`,
            title: item.data.title || "Untitled",
            endsAtMs,
          };
        }

        continue;
      }

      const diffMs = startsAtMs - currentTimeMs;
      if (diffMs <= 0 || diffMs > UPCOMING_MEETING_VISIBLE_WINDOW_MS) {
        continue;
      }

      if (!nearest || diffMs < nearest.diffMs) {
        nearest = {
          itemKey: `${item.type}-${item.id}`,
          title: item.data.title || "Untitled",
          diffMs,
        };
      }
    }
  }

  if (active) {
    return {
      itemKey: active.itemKey,
      label: activeLabel,
      progress: 1,
      title: active.title,
    };
  }

  if (!nearest) {
    return null;
  }

  return {
    itemKey: nearest.itemKey,
    label: formatLabel(nearest.diffMs),
    progress: getUpcomingMeetingProgress(nearest.diffMs),
    title: nearest.title,
  };
}

function getUpcomingMeetingProgress(diffMs: number): number {
  return Math.max(0, Math.min(diffMs / UPCOMING_MEETING_VISIBLE_WINDOW_MS, 1));
}

function upcomingMeetingStatusesAreEqual(
  left: SidebarUpcomingMeetingStatus | null,
  right: SidebarUpcomingMeetingStatus | null,
) {
  return (
    left === right ||
    (left !== null &&
      right !== null &&
      left.itemKey === right.itemKey &&
      left.label === right.label &&
      left.title === right.title &&
      left.progress === right.progress)
  );
}

export function useUpcomingMeetingLabelFormatter(): (diffMs: number) => string {
  const { t } = useLingui();

  return useCallback(
    (diffMs: number) => {
      const totalSeconds = Math.max(1, Math.floor(diffMs / 1000));
      const minutes = Math.floor(totalSeconds / 60);
      const seconds = totalSeconds % 60;

      if (minutes < 1) {
        return t`In ${totalSeconds}s`;
      }

      return t`In ${minutes}m ${seconds}s`;
    },
    [t],
  );
}

function formatUpcomingMeetingLabelEnglish(diffMs: number): string {
  const totalSeconds = Math.max(1, Math.floor(diffMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;

  if (minutes < 1) {
    return `In ${totalSeconds}s`;
  }

  return `In ${minutes}m ${seconds}s`;
}
