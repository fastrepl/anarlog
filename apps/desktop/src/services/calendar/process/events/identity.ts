import type { CalendarProviderType } from "@anlg/plugin-calendar";

import type { IncomingEvent } from "../../fetch/types";

type EventIdentity = {
  tracking_id_event: string;
  title?: string;
  started_at?: string;
  ended_at?: string;
  is_all_day: boolean;
};

export function calendarEventKey(
  provider: CalendarProviderType,
  calendarId: string,
  event: EventIdentity,
): string {
  if (provider === "apple") {
    const title = event.title?.trim();
    const startedAt = normalizedInstant(event.started_at);
    const endedAt = normalizedInstant(event.ended_at);
    if (title !== undefined && startedAt && endedAt) {
      return [
        calendarId,
        "visible",
        title,
        startedAt,
        endedAt,
        event.is_all_day ? "all-day" : "timed",
      ].join("\u0000");
    }
  }

  return trackingEventKey(calendarId, event.tracking_id_event);
}

export function calendarEventKeys(
  provider: CalendarProviderType,
  calendarId: string,
  event: EventIdentity,
): string[] {
  const trackingKey = trackingEventKey(calendarId, event.tracking_id_event);
  const visibleKey = calendarEventKey(provider, calendarId, event);
  return trackingKey === visibleKey ? [trackingKey] : [trackingKey, visibleKey];
}

export function buildIncomingEventIndex(
  provider: CalendarProviderType,
  incoming: IncomingEvent[],
  calendarTrackingIdToId: Map<string, string>,
): {
  canonical: Map<string, IncomingEvent>;
  expanded: Map<string, IncomingEvent>;
} {
  const canonical = new Map<string, IncomingEvent>();
  for (const event of incoming) {
    const calendarId = calendarTrackingIdToId.get(event.tracking_id_calendar);
    if (!calendarId) continue;

    const key = calendarEventKey(provider, calendarId, event);
    const current = canonical.get(key);
    if (!current || isNewerProviderRecord(event, current)) {
      canonical.set(key, event);
    }
  }

  const expanded = new Map<string, IncomingEvent>();
  for (const event of incoming) {
    const calendarId = calendarTrackingIdToId.get(event.tracking_id_calendar);
    if (!calendarId) continue;
    const selected = canonical.get(
      calendarEventKey(provider, calendarId, event),
    );
    if (!selected) continue;
    for (const key of calendarEventKeys(provider, calendarId, event)) {
      expanded.set(key, selected);
    }
  }

  return { canonical, expanded };
}

function trackingEventKey(calendarId: string, trackingId: string): string {
  return `${calendarId}\u0000tracking\u0000${trackingId}`;
}

function normalizedInstant(value: string | undefined): string | undefined {
  if (!value) return undefined;
  const instant = Date.parse(value);
  return Number.isFinite(instant) ? String(instant) : value;
}

function isNewerProviderRecord(
  candidate: IncomingEvent,
  current: IncomingEvent,
): boolean {
  const candidateModified = Date.parse(candidate.provider_modified_at ?? "");
  const currentModified = Date.parse(current.provider_modified_at ?? "");

  if (Number.isFinite(candidateModified) && Number.isFinite(currentModified)) {
    if (candidateModified !== currentModified) {
      return candidateModified > currentModified;
    }
  } else if (Number.isFinite(candidateModified)) {
    return true;
  } else if (Number.isFinite(currentModified)) {
    return false;
  }

  return (
    candidate.tracking_id_event.localeCompare(current.tracking_id_event) > 0
  );
}
