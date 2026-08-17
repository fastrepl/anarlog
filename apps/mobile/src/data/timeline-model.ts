export type TimelineSession = {
  id: string;
  title: string;
  startedAt: string;
};

export type SessionListItem =
  | { type: "group"; key: string; label: "Upcoming" | "Past" }
  | { type: "header"; key: string; label: string }
  | { type: "session"; key: string; session: TimelineSession };

export type TimelineRow = {
  id: string;
  title: string;
  created_at: string;
  event_json: string;
};

const DAY = 24 * 60 * 60 * 1000;
const MINUTE = 60 * 1000;

function startOfDay(timestamp: number): number {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

export function dayLabel(iso: string, now = Date.now()): string {
  const timestamp = new Date(iso).getTime();
  if (!Number.isFinite(timestamp)) return "Date unavailable";
  const today = startOfDay(now);
  const day = startOfDay(timestamp);
  if (day === today) return "Today";
  if (day === today + DAY) return "Tomorrow";
  if (day === today - DAY) return "Yesterday";
  const date = new Date(day);
  const sameYear = date.getFullYear() === new Date(today).getFullYear();
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    ...(sameYear ? {} : { year: "numeric" }),
  });
}

export function relativeLabel(iso: string): string {
  const timestamp = new Date(iso).getTime();
  if (!Number.isFinite(timestamp)) return "date unavailable";
  const diff = timestamp - Date.now();
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
  now = Date.now(),
): SessionListItem[] {
  const items: SessionListItem[] = [];
  const appendGroup = (
    label: "Upcoming" | "Past",
    group: TimelineSession[],
  ) => {
    if (group.length === 0) return;
    const groupKey = label.toLowerCase();
    items.push({ type: "group", key: `group-${groupKey}`, label });
    let currentLabel: string | null = null;
    for (const session of group) {
      const sessionDayLabel = dayLabel(session.startedAt, now);
      if (sessionDayLabel !== currentLabel) {
        items.push({
          type: "header",
          key: `header-${groupKey}-${sessionDayLabel}`,
          label: sessionDayLabel,
        });
        currentLabel = sessionDayLabel;
      }
      items.push({ type: "session", key: session.id, session });
    }
  };

  const withTimestamps = sessions.map((session) => {
    const timestamp = new Date(session.startedAt).getTime();
    return {
      session,
      timestamp: Number.isFinite(timestamp) ? timestamp : null,
    };
  });
  const upcoming = withTimestamps
    .filter(
      (item): item is { session: TimelineSession; timestamp: number } =>
        item.timestamp !== null && item.timestamp > now,
    )
    .sort((a, b) => a.timestamp - b.timestamp)
    .map((item) => item.session);
  const past = withTimestamps
    .filter((item) => item.timestamp === null || item.timestamp <= now)
    .sort((a, b) => (b.timestamp ?? -Infinity) - (a.timestamp ?? -Infinity))
    .map((item) => item.session);

  appendGroup("Upcoming", upcoming);
  appendGroup("Past", past);
  return items;
}

export function nextTimelineRefreshAt(
  sessions: TimelineSession[],
  now = Date.now(),
): number {
  const nextMinute = now - (now % MINUTE) + MINUTE + 1;
  const nextMeeting = sessions.reduce((earliest, session) => {
    const timestamp = new Date(session.startedAt).getTime();
    return Number.isFinite(timestamp) && timestamp > now
      ? Math.min(earliest, timestamp + 1)
      : earliest;
  }, Number.POSITIVE_INFINITY);
  return Math.min(nextMinute, nextMeeting);
}

function sessionStartedAt(row: TimelineRow): string {
  if (row.event_json) {
    try {
      const event: unknown = JSON.parse(row.event_json);
      if (
        event &&
        typeof event === "object" &&
        typeof (event as { started_at?: unknown }).started_at === "string" &&
        (event as { started_at: string }).started_at !== "" &&
        Number.isFinite(
          new Date((event as { started_at: string }).started_at).getTime(),
        )
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
