import { useMemo, useSyncExternalStore } from "react";

import { useLiveQuery } from "@/db";

import {
  buildSessionList,
  mapTimelineRows,
  nextTimelineRefreshAt,
  type SessionListItem,
  type TimelineRow,
  type TimelineSession,
} from "./timeline-model";

export * from "./timeline-model";

const TIMELINE_SQL = `
SELECT
  sessions.id,
  sessions.title,
  sessions.created_at,
  sessions.event_json,
  sessions.folder_path,
  COALESCE((
    SELECT json_group_array(name)
    FROM (
      SELECT DISTINCT tags.name AS name
      FROM session_tags
      JOIN tags ON tags.id = session_tags.tag_id
      WHERE session_tags.session_id = sessions.id
        AND session_tags.deleted_at IS NULL
        AND tags.deleted_at IS NULL
        AND trim(tags.name) <> ''
      ORDER BY tags.name COLLATE NOCASE
    )
  ), '[]') AS tags_json
FROM sessions
WHERE sessions.deleted_at IS NULL
ORDER BY sessions.created_at DESC
`;

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

export function useTimelineSessions(): {
  items: SessionListItem[];
  isLoading: boolean;
} {
  const { data, isLoading } = useLiveQuery<TimelineRow, TimelineSession[]>({
    sql: TIMELINE_SQL,
    mapRows: mapTimelineRows,
  });
  const sessions = useMemo(() => data ?? [], [data]);
  const clock = useMemo(() => createTimelineClock(sessions), [sessions]);
  const now = useSyncExternalStore(
    clock.subscribe,
    clock.getSnapshot,
    clock.getSnapshot,
  );
  const items = useMemo(() => buildSessionList(sessions, now), [sessions, now]);
  return { items, isLoading };
}
