import { useMemo } from "react";

import {
  commands as trayCommands,
  type TrayScheduleEvent,
} from "@hypr/plugin-tray";
import { getCurrentWebviewWindowLabel } from "@hypr/plugin-windows";
import { addDays, safeParseDate, startOfDay, TZDate } from "@hypr/utils";

import { useIgnoredEvents } from "~/calendar/ignored-events";
import { useTimelineEventsTable } from "~/calendar/queries";
import { useConfigValue } from "~/shared/config";
import { useCurrentDay } from "~/shared/hooks/useCurrentDay";
import { useMountEffect } from "~/shared/hooks/useMountEffect";
import type { TimelineEventRow } from "~/sidebar/timeline/utils";

const PUBLISHED_SCHEDULE_HORIZON_MS = 7 * 24 * 60 * 60 * 1000;

export function buildTrayScheduleEvents(
  rows: Record<string, TimelineEventRow> | null | undefined,
  isIgnored: (
    trackingId: string | null | undefined,
    recurrenceSeriesId: string | null | undefined,
  ) => boolean,
  nowMs = Date.now(),
  timezone?: string,
  locale?: string,
): TrayScheduleEvent[] {
  const upperBoundMs = nowMs + PUBLISHED_SCHEDULE_HORIZON_MS;
  const timeFormatter = new Intl.DateTimeFormat(locale, {
    hour: "numeric",
    minute: "2-digit",
    timeZone: timezone,
  });

  return Object.values(rows ?? {})
    .flatMap((row): TrayScheduleEvent[] => {
      if (
        row.is_all_day ||
        isIgnored(row.tracking_id_event, row.recurrence_series_id)
      ) {
        return [];
      }

      const start = safeParseDate(row.started_at);
      if (!start) {
        return [];
      }

      const startsAtMs = start.getTime();
      if (startsAtMs > upperBoundMs) {
        return [];
      }

      const parsedEnd = safeParseDate(row.ended_at);
      const endsAtMs =
        parsedEnd && parsedEnd.getTime() > startsAtMs
          ? parsedEnd.getTime()
          : null;
      if (
        (endsAtMs !== null && endsAtMs <= nowMs) ||
        (endsAtMs === null && startsAtMs <= nowMs)
      ) {
        return [];
      }

      const dayStart = startOfDay(toTimezone(start, timezone));
      const previousDayStart = startOfDay(addDays(dayStart, -1));
      const timeLabel = endsAtMs
        ? `${timeFormatter.format(start)} – ${timeFormatter.format(endsAtMs)}`
        : timeFormatter.format(start);

      return [
        {
          title: row.title?.trim() || "Untitled event",
          startsAtMs,
          endsAtMs,
          dayStartMs: dayStart.getTime(),
          previousDayStartMs: previousDayStart.getTime(),
          timeLabel,
        },
      ];
    })
    .sort(
      (left, right) =>
        left.startsAtMs - right.startsAtMs ||
        left.title.localeCompare(right.title),
    );
}

export function TrayScheduleSync() {
  const timelineEventsTable = useTimelineEventsTable();
  const { isIgnored } = useIgnoredEvents();
  const timezone = useConfigValue("timezone") || undefined;
  const currentDay = useCurrentDay(timezone);
  const events = useMemo(
    () =>
      buildTrayScheduleEvents(
        timelineEventsTable,
        isIgnored,
        Date.now(),
        timezone,
      ),
    [currentDay, isIgnored, timelineEventsTable, timezone],
  );

  return <TraySchedulePublisher key={JSON.stringify(events)} events={events} />;
}

function toTimezone(date: Date, timezone?: string): Date {
  return timezone ? new TZDate(date, timezone) : date;
}

function TraySchedulePublisher({ events }: { events: TrayScheduleEvent[] }) {
  useMountEffect(() => {
    if (getCurrentWebviewWindowLabel() !== "main") {
      return;
    }

    void trayCommands
      .setTraySchedule(events)
      .then((result) => {
        if (result.status === "error") {
          console.error("[tray] failed to publish schedule", result.error);
        }
      })
      .catch((error) => {
        console.error("[tray] failed to publish schedule", error);
      });
  });

  return null;
}
