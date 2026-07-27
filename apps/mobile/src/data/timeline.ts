import { useMemo } from "react";

import { useLiveQuery } from "@/db";

export type TimelineSession = {
  id: string;
  title: string;
  startedAt: string;
};

export type SessionListItem =
  | { type: "header"; key: string; label: string }
  | { type: "session"; key: string; session: TimelineSession }
  | { type: "now"; key: string };

const DAY = 24 * 60 * 60 * 1000;

function startOfDay(timestamp: number): number {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

export function dayLabel(iso: string): string {
  const today = startOfDay(Date.now());
  const day = startOfDay(new Date(iso).getTime());
  if (day === today) return "Today";
  if (day === today + DAY) return "Tomorrow";
  if (day === today - DAY) return "Yesterday";
  const date = new Date(day);
  // Section labels double as React keys, so an older year has to be spelled out
  // or the same calendar day across years collides.
  const sameYear = date.getFullYear() === new Date(today).getFullYear();
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

export function relativeLabel(iso: string): string {
  const diff = new Date(iso).getTime() - Date.now();
  const minutes = Math.round(Math.abs(diff) / 60000);
  if (minutes < 1) return "now";
  const unit =
    minutes < 60
      ? `${minutes} min${minutes === 1 ? "" : "s"}`
      : `${Math.round(minutes / 60)} hour${Math.round(minutes / 60) === 1 ? "" : "s"}`;
  return diff > 0 ? `${unit} later` : `${unit} ago`;
}

export function buildSessionList(
  sessions: TimelineSession[],
): SessionListItem[] {
  const sorted = [...sessions].sort(
    (a, b) => new Date(b.startedAt).getTime() - new Date(a.startedAt).getTime(),
  );

  const items: SessionListItem[] = [];
  const nowIso = new Date().toISOString();
  let currentLabel: string | null = null;
  let nowInserted = false;

  for (const session of sorted) {
    const label = dayLabel(session.startedAt);
    if (label !== currentLabel) {
      items.push({ type: "header", key: `header-${label}`, label });
      currentLabel = label;
    }
    if (!nowInserted && session.startedAt <= nowIso && label === "Today") {
      items.push({ type: "now", key: "now" });
      nowInserted = true;
    }
    items.push({ type: "session", key: session.id, session });
  }

  return items;
}

const TIMELINE_SQL = `
SELECT id, title, created_at, event_json
FROM sessions
WHERE deleted_at IS NULL
ORDER BY created_at DESC
`;

export type TimelineRow = {
  id: string;
  title: string;
  created_at: string;
  event_json: string;
};

function sessionStartedAt(row: TimelineRow): string {
  if (row.event_json) {
    try {
      const event: unknown = JSON.parse(row.event_json);
      if (
        event &&
        typeof event === "object" &&
        typeof (event as { started_at?: unknown }).started_at === "string" &&
        (event as { started_at: string }).started_at !== ""
      ) {
        return (event as { started_at: string }).started_at;
      }
    } catch {
      return row.created_at;
    }
  }
  return row.created_at;
}

export function mapTimelineRows(rows: TimelineRow[]): TimelineSession[] {
  return rows.map((row) => ({
    id: row.id,
    title: row.title,
    startedAt: sessionStartedAt(row),
  }));
}

export function useTimelineSessions(): {
  items: SessionListItem[];
  isLoading: boolean;
} {
  const { data, isLoading } = useLiveQuery<TimelineRow, TimelineSession[]>({
    sql: TIMELINE_SQL,
    mapRows: mapTimelineRows,
  });
  const items = useMemo(() => buildSessionList(data ?? []), [data]);
  return { items, isLoading };
}
