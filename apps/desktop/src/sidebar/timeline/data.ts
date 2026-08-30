import { useMemo } from "react";

import { useSmartCurrentTime } from "./realtime";
import {
  buildTimelineBuckets,
  deriveTimelineWindowData,
  filterTimelineTablesByFolder,
  getItemTimestamp,
  type TimelineBucket,
  type TimelineEventsTable,
  type TimelineSessionsTable,
} from "./utils";

export function getFallbackIndicatorIndex(
  buckets: TimelineBucket[],
  nowMs: number,
) {
  let staleFutureBoundary: number | null = null;

  for (let index = 0; index < buckets.length; index++) {
    const bucket = buckets[index];
    const firstItem = bucket?.items[0];
    if (!bucket || !firstItem) {
      continue;
    }

    const itemDate = getItemTimestamp(firstItem);
    if (!itemDate || itemDate.getTime() >= nowMs) {
      continue;
    }

    if (isFutureBucketLabel(bucket.label)) {
      staleFutureBoundary = index + 1;
      continue;
    }

    return staleFutureBoundary ?? index;
  }

  return staleFutureBoundary ?? -1;
}

function isFutureBucketLabel(label: string) {
  return (
    label === "Tomorrow" ||
    label === "next week" ||
    label === "next month" ||
    label.startsWith("in ")
  );
}

export function useTimelineData({
  folderFilter = null,
  isEventIgnored,
  showIgnored,
  timelineEventsTable,
  timelineSessionsTable,
  timezone,
}: {
  folderFilter?: string | null;
  isEventIgnored: (
    trackingId: string | null | undefined,
    recurrenceSeriesId: string | null | undefined,
  ) => boolean;
  showIgnored: boolean;
  timelineEventsTable: TimelineEventsTable;
  timelineSessionsTable: TimelineSessionsTable;
  timezone?: string;
}): {
  buckets: TimelineBucket[];
  hasMoreFutureItems: boolean;
} {
  const folderScopedTables = useMemo(
    () =>
      filterTimelineTablesByFolder({
        folderFilter,
        timelineEventsTable,
        timelineSessionsTable,
      }),
    [folderFilter, timelineEventsTable, timelineSessionsTable],
  );
  const windowData = useMemo(
    () =>
      deriveTimelineWindowData({
        isEventIgnored,
        showIgnored,
        timelineEventsTable: folderScopedTables.timelineEventsTable,
        timelineSessionsTable: folderScopedTables.timelineSessionsTable,
        timezone,
      }),
    [folderScopedTables, isEventIgnored, showIgnored, timezone],
  );
  const currentTimeMs = useSmartCurrentTime(
    windowData.timelineEventsTable,
    windowData.timelineSessionsTable,
  );

  return useMemo(() => {
    const buckets = buildTimelineBuckets({
      timelineEventsTable: windowData.timelineEventsTable,
      timelineSessionsTable: windowData.timelineSessionsTable,
      timezone,
    });

    return {
      buckets,
      hasMoreFutureItems: windowData.hasMoreFutureItems,
    };
  }, [windowData, currentTimeMs, timezone]);
}
