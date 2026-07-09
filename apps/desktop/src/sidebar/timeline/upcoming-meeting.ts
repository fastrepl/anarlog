import { useLingui } from "@lingui/react/macro";
import { useCallback, useMemo } from "react";

import { useCurrentTimeMs } from "./realtime";
import {
  buildTimelineBuckets,
  deriveTimelineWindowData,
  getItemTimeRange,
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

    return getUpcomingMeetingStatus(
      buckets,
      currentTimeMs,
      formatLabel,
      t`Now`,
    );
  }, [
    currentTimeMs,
    formatLabel,
    isIgnored,
    showIgnored,
    timelineEventsTable,
    timelineSessionsTable,
    t,
    timezone,
  ]);
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
