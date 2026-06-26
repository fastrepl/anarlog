import { useLingui } from "@lingui/react/macro";
import { useCallback, useMemo } from "react";

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

export function useSidebarUpcomingMeetingStatus({
  showIgnored = false,
}: {
  showIgnored?: boolean;
} = {}): SidebarUpcomingMeetingStatus | null {
  const timezone = useConfigValue("timezone") || undefined;
  const { isIgnored } = useIgnoredEvents();
  const formatLabel = useUpcomingMeetingLabelFormatter();
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
      showIgnored,
      timelineEventsTable,
      timelineSessionsTable,
      timezone,
    });
    const buckets = buildTimelineBuckets({
      timelineEventsTable: windowData.timelineEventsTable,
      timelineSessionsTable: windowData.timelineSessionsTable,
      timezone,
    });

    return getUpcomingMeetingStatus(buckets, currentTimeMs, formatLabel);
  }, [
    currentTimeMs,
    formatLabel,
    isIgnored,
    showIgnored,
    timelineEventsTable,
    timelineSessionsTable,
    timezone,
  ]);
}

export function getUpcomingMeetingStatus(
  buckets: TimelineBucket[],
  currentTimeMs: number,
  formatLabel: (diffMs: number) => string = formatUpcomingMeetingLabelEnglish,
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
    label: formatLabel(nearest.diffMs),
    title: nearest.title,
  };
}

export function useUpcomingMeetingLabelFormatter(): (diffMs: number) => string {
  const { t } = useLingui();

  return useCallback(
    (diffMs: number) => {
      const totalSeconds = Math.max(1, Math.floor(diffMs / 1000));
      const minutes = Math.floor(totalSeconds / 60);

      if (minutes < 1) {
        return totalSeconds === 1
          ? t`In ${totalSeconds} second`
          : t`In ${totalSeconds} seconds`;
      }

      return minutes === 1 ? t`In ${minutes} minute` : t`In ${minutes} minutes`;
    },
    [t],
  );
}

function formatUpcomingMeetingLabelEnglish(diffMs: number): string {
  const totalSeconds = Math.max(1, Math.floor(diffMs / 1000));
  const minutes = Math.floor(totalSeconds / 60);

  if (minutes < 1) {
    return `In ${totalSeconds} ${totalSeconds === 1 ? "second" : "seconds"}`;
  }

  return `In ${minutes} ${minutes === 1 ? "minute" : "minutes"}`;
}
