import type { SessionEvent } from "@anlg/store";

export function getSessionEvent(session: {
  event_json?: string | null;
}): SessionEvent | null {
  const eventJson = session.event_json;
  if (!eventJson) return null;
  try {
    return JSON.parse(eventJson) as SessionEvent;
  } catch {
    return null;
  }
}

export function sessionSearchTimestamp(
  eventJson: string | null | undefined,
  createdAt: string,
): number {
  const event = getSessionEvent({ event_json: eventJson });
  const fromEvent = toEpochMs(event?.started_at);
  if (fromEvent > 0) {
    return fromEvent;
  }

  return toEpochMs(createdAt);
}

function toEpochMs(value: unknown): number {
  if (typeof value === "number" && Number.isFinite(value)) {
    return Math.trunc(value);
  }

  if (typeof value !== "string" || !value.trim()) {
    return 0;
  }

  const parsed = Date.parse(value);
  if (!Number.isNaN(parsed)) {
    return parsed;
  }

  const numeric = Number(value);
  return Number.isFinite(numeric) ? Math.trunc(numeric) : 0;
}
