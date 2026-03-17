import type { SessionEvent } from "@hypr/store";

import type * as main from "~/store/tinybase/store/main";

type Store = NonNullable<ReturnType<typeof main.UI.useStore>>;

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

export function getSessionEventById(
  store: Store,
  sessionId: string,
): SessionEvent | null {
  const row = store.getRow("sessions", sessionId);
  if (!row) return null;
  return getSessionEvent(row);
}

export function findSessionByTrackingId(
  store: Store,
  trackingId: string,
): string | null {
  let found: string | null = null;
  store.forEachRow("sessions", (rowId, _forEachCell) => {
    if (found) return;
    const sessionEvent = getSessionEventById(store, rowId);
    if (!sessionEvent) return;
    if (sessionEvent.tracking_id === trackingId) {
      found = rowId;
    }
  });
  return found;
}

export function findSessionByEventId(
  store: Store,
  eventId: string,
): string | null {
  if (!store.hasRow("events", eventId)) return null;
  const event = store.getRow("events", eventId);
  if (!event) return null;
  const trackingId = event.tracking_id_event;
  if (!trackingId) return null;
  return findSessionByTrackingId(store, trackingId);
}
