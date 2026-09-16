import type { Ctx } from "../../ctx";
import type { IncomingEvent } from "../../fetch/types";

export function eventTrackingIds(event: IncomingEvent): string[] {
  return [event.tracking_id_event, ...(event.legacy_tracking_ids ?? [])];
}

export function eventKey(calendarId: string, trackingId: string): string {
  return JSON.stringify([calendarId, trackingId]);
}

export function indexIncomingEvents(ctx: Ctx, incoming: IncomingEvent[]) {
  const index = new Map<string, IncomingEvent>();
  const ambiguous = new Set<string>();
  for (const event of incoming) {
    const calendarId = ctx.calendarTrackingIdToId.get(
      event.tracking_id_calendar,
    );
    if (!calendarId) continue;
    for (const trackingId of eventTrackingIds(event)) {
      const key = eventKey(calendarId, trackingId);
      const current = index.get(key);
      if (current && current.tracking_id_event !== event.tracking_id_event) {
        ambiguous.add(key);
      }
      index.set(key, event);
    }
  }
  for (const key of ambiguous) index.delete(key);
  return index;
}
