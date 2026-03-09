import { forwardRef, useEffect, useMemo, useState } from "react";

import { TZDate, format, safeParseDate } from "@hypr/utils";

import type { TimelineEventsTable, TimelineSessionsTable } from "./utils";

import { getSessionEvent } from "~/session/utils";

export const CurrentTimeIndicator = forwardRef<
  HTMLDivElement,
  { timezone?: string; variant?: "seam" | "inside"; progress?: number }
>(function CurrentTimeIndicator(
  { timezone, variant = "seam", progress = 0.5 },
  ref,
) {
  const currentTimeMs = useCurrentTimeMs();
  const label = useMemo(() => {
    const now = timezone
      ? new TZDate(new Date(currentTimeMs), timezone)
      : new Date(currentTimeMs);
    return format(now, "h:mm a").toUpperCase();
  }, [currentTimeMs, timezone]);

  return (
    <div
      ref={ref}
      aria-hidden
      className={
        variant === "inside"
          ? "group absolute inset-x-0 z-20 h-px"
          : "group relative z-20 h-px"
      }
      style={variant === "inside" ? { top: `${progress * 100}%` } : undefined}
    >
      <div className="absolute top-0 right-3 left-3 -translate-y-1/2">
        <div className="absolute top-1/2 right-0 left-0 h-px -translate-y-1/2 bg-red-400/90 mix-blend-multiply" />
        <div className="relative flex h-5 items-center justify-center">
          <div className="rounded-full bg-red-500 px-2 py-0.5 font-mono text-[11px] font-semibold text-white opacity-0 shadow-xs transition-opacity group-hover:opacity-100">
            {label}
          </div>
        </div>
      </div>
    </div>
  );
});

export function useCurrentTimeMs() {
  const [now, setNow] = useState(() => new Date().getTime());

  useEffect(() => {
    const update = () => {
      const now = new Date().getTime();
      setNow(now);
    };

    update();

    const interval = setInterval(update, 60_000);
    return () => clearInterval(interval);
  }, []);

  return now;
}

export function useSmartCurrentTime(
  eventsTable: TimelineEventsTable,
  sessionsTable: TimelineSessionsTable,
) {
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    let timeoutId: NodeJS.Timeout | undefined;

    const scheduleNext = () => {
      const currentTime = Date.now();
      setNow(currentTime);

      const importantTimes: number[] = [];

      if (eventsTable) {
        Object.values(eventsTable).forEach((event) => {
          const startTime = safeParseDate(event.started_at);
          const endTime = safeParseDate(event.ended_at);

          if (startTime && startTime.getTime() > currentTime) {
            importantTimes.push(startTime.getTime());
          }
          if (endTime && endTime.getTime() > currentTime) {
            importantTimes.push(endTime.getTime());
          }
        });
      }

      if (sessionsTable) {
        Object.values(sessionsTable).forEach((session) => {
          const time = safeParseDate(
            getSessionEvent(session)?.started_at ?? session.created_at,
          );
          if (time && time.getTime() > currentTime) {
            importantTimes.push(time.getTime());
          }
        });
      }

      let nextUpdateDelay: number;
      if (importantTimes.length > 0) {
        const nextTime = Math.min(...importantTimes);
        const msUntilNext = nextTime - currentTime;
        nextUpdateDelay = Math.max(100, Math.min(msUntilNext + 100, 60_000));
      } else {
        nextUpdateDelay = 60_000;
      }

      timeoutId = setTimeout(scheduleNext, nextUpdateDelay);
    };

    scheduleNext();

    return () => {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
    };
  }, [eventsTable, sessionsTable]);

  return now;
}
