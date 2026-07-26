export type SessionRow = {
  id: string;
  title: string;
  startedAt: string;
};

const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;

const now = Date.now();

export const mockSessions: SessionRow[] = [
  {
    id: "s-tomorrow-1",
    title: "Design review",
    startedAt: new Date(now + DAY).toISOString(),
  },
  {
    id: "s-upcoming-1",
    title: "Weekly sync",
    startedAt: new Date(now + 10 * 60 * 1000).toISOString(),
  },
  {
    id: "s-today-1",
    title: "1:1 with Sam",
    startedAt: new Date(now - 2 * HOUR).toISOString(),
  },
  {
    id: "s-today-2",
    title: "",
    startedAt: new Date(now - 5 * HOUR).toISOString(),
  },
  {
    id: "s-yesterday-1",
    title: "Product standup",
    startedAt: new Date(now - DAY).toISOString(),
  },
];

export function getSession(id: string): SessionRow | undefined {
  return mockSessions.find((session) => session.id === id);
}

function startOfDay(timestamp: number): number {
  const date = new Date(timestamp);
  date.setHours(0, 0, 0, 0);
  return date.getTime();
}

export function dayLabel(startedAt: string): string {
  const today = startOfDay(Date.now());
  const day = startOfDay(new Date(startedAt).getTime());
  if (day === today) return "Today";
  if (day === today + DAY) return "Tomorrow";
  if (day === today - DAY) return "Yesterday";
  return new Date(day).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
  });
}

export function relativeLabel(startedAt: string): string {
  const diff = new Date(startedAt).getTime() - Date.now();
  const minutes = Math.round(Math.abs(diff) / 60000);
  if (minutes < 1) return "now";
  const unit =
    minutes < 60
      ? `${minutes} min${minutes === 1 ? "" : "s"}`
      : `${Math.round(minutes / 60)} hour${Math.round(minutes / 60) === 1 ? "" : "s"}`;
  return diff > 0 ? `${unit} later` : `${unit} ago`;
}

export type SessionListItem =
  | { type: "header"; key: string; label: string }
  | { type: "session"; key: string; session: SessionRow }
  | { type: "now"; key: string };

export function buildSessionList(sessions: SessionRow[]): SessionListItem[] {
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
