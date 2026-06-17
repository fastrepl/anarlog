import { useMemo } from "react";

import { useCurrentTimeMs } from "./realtime";
import {
  buildTimelineBuckets,
  deriveTimelineWindowData,
  getItemTimestamp,
  type TimelineBucket,
} from "./utils";

import { useConfigValue } from "~/shared/config";
import { useIgnoredEvents } from "~/store/tinybase/hooks";
import * as main from "~/store/tinybase/store/main";

const UPCOMING_MEETING_VISIBLE_WINDOW_MS = 5 * 60 * 1000;
const UPCOMING_MEETING_STATUS_TICK_MS = 1000;

export type SidebarUpcomingMeetingStatus = {
  itemKey: string;
  label: string;
  title: string;
};

export function useSidebarUpcomingMeetingStatus(): SidebarUpcomingMeetingStatus | null {
  const timezone = useConfigValue("timezone") || undefined;
  const { isIgnored } = useIgnoredEvents();
  const timelineEventsTable = main.UI.useResultTable(
    main.QUERIES.timelineEvents,
    main.STORE_ID,
  );
  const timelineSessionsTable = main.UI.useResultTable(
    main.QUERIES.timelineSessions,
    main.STORE_ID,
  );
  const currentTimeMs = useCurrentTimeMs(UPCOMING_MEETING_STATUS_TICK_MS);

  return useMemo(() => {
    const windowData = deriveTimelineWindowData({
      isEventIgnored: isIgnored,
      showIgnored: false,
      timelineEventsTable,
      timelineSessionsTable,
      timezone,
    });
    const buckets = buildTimelineBuckets({
      timelineEventsTable: windowData.timelineEventsTable,
      timelineSessionsTable: windowData.timelineSessionsTable,
      timezone,
    });

    return getUpcomingMeetingStatus(buckets, currentTimeMs);
  }, [
    currentTimeMs,
    isIgnored,
    timelineEventsTable,
    timelineSessionsTable,
    timezone,
  ]);
}

export function getUpcomingMeetingStatus(
  buckets: TimelineBucket[],
  currentTimeMs: number,
): SidebarUpcomingMeetingStatus | null {
  let nearest: { itemKey: string; title: string; diffMs: number } | null = null;

  for (const bucket of buckets) {
    for (const item of bucket.items) {
      if (item.type === "event" && item.data.is_all_day) {
        continue;
      }

      const timestamp = getItemTimestamp(item);
      if (!timestamp) {
        continue;
      }

      const diffMs = timestamp.getTime() - currentTimeMs;
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

  if (!nearest) {
    return null;
  }

  return {
    itemKey: nearest.itemKey,
    label: formatUpcomingMeetingLabel(nearest.diffMs),
    title: nearest.title,
  };
}

function formatUpcomingMeetingLabel(diffMs: number): string {
  const totalSeconds = Math.max(1, Math.floor(diffMs / 1000));
  if (totalSeconds < 60) {
    return `Starts in ${totalSeconds}s`;
  }

  return `Starts in ${Math.floor(totalSeconds / 60)}m`;
}
