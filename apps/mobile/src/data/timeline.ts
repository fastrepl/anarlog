import { useMemo, useRef, useState, useSyncExternalStore } from "react";

import { useLiveQuery } from "@/db";
import { useLanguage } from "@/i18n";

import {
  buildSessionList,
  mapTimelineRows,
  nextTimelineRefreshAt,
  type TimelineRow,
  type TimelineSession,
} from "./timeline-model";
import { TIMELINE_PAGE_SIZE, TIMELINE_SQL } from "./timeline-query";

export * from "./timeline-model";

const MAX_TIMER_DELAY_MS = 2_147_483_647;

function createTimelineClock(sessions: TimelineSession[]) {
  let snapshot = Date.now();
  let timer: ReturnType<typeof setTimeout> | null = null;
  const listeners = new Set<() => void>();

  const schedule = () => {
    const current = Date.now();
    const delay = Math.min(
      Math.max(1, nextTimelineRefreshAt(sessions, current) - current),
      MAX_TIMER_DELAY_MS,
    );
    timer = setTimeout(() => {
      snapshot = Date.now();
      for (const listener of listeners) listener();
      if (listeners.size > 0) schedule();
    }, delay);
  };

  return {
    getSnapshot: () => snapshot,
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      if (listeners.size === 1) schedule();
      return () => {
        listeners.delete(listener);
        if (listeners.size === 0 && timer) {
          clearTimeout(timer);
          timer = null;
        }
      };
    },
  };
}

export function useTimelineSessions() {
  const { language } = useLanguage();
  const [limit, setLimit] = useState(TIMELINE_PAGE_SIZE);
  const [orderedAt, setOrderedAt] = useState(Date.now);
  const previousData = useRef<TimelineSession[]>([]);
  const { data, isLoading, error } = useLiveQuery<
    TimelineRow,
    TimelineSession[]
  >({
    sql: TIMELINE_SQL,
    params: [new Date(orderedAt).toISOString(), limit + 1],
    mapRows: mapTimelineRows,
  });
  // Keep the current rows mounted while the larger live query subscribes.
  if (data !== undefined) previousData.current = data;
  const rows = data ?? previousData.current;
  const sessions = useMemo(() => rows.slice(0, limit), [rows, limit]);
  const clock = useMemo(() => createTimelineClock(sessions), [sessions]);
  const now = useSyncExternalStore(
    clock.subscribe,
    clock.getSnapshot,
    clock.getSnapshot,
  );
  const locale = language === "vi" ? "vi-VN" : "en-US";
  const items = useMemo(
    () => buildSessionList(sessions, now, locale),
    [sessions, now, locale],
  );
  const hasMore = rows.length > limit;

  // A meeting crossing into the past can change which rows belong in the window.
  if (
    sessions.some((session) => {
      const startedAt = new Date(session.startedAt).getTime();
      return startedAt > orderedAt && startedAt <= now;
    })
  )
    setOrderedAt(now);

  return {
    items,
    isLoading,
    error,
    hasMore,
    loadMore: () => {
      if (!isLoading && !error && hasMore) setLimit(limit + TIMELINE_PAGE_SIZE);
    },
    retry: () => setOrderedAt(Date.now()),
  };
}
