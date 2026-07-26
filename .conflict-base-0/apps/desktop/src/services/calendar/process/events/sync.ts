import type { Ctx } from "../../ctx";
import type { EventsSyncInput, EventsSyncOutput } from "./types";

export function syncEvents(
  ctx: Ctx,
  { incoming, existing, incomingParticipants }: EventsSyncInput,
): EventsSyncOutput {
  const out: EventsSyncOutput = {
    toDelete: [],
    toUpdate: [],
    toAdd: [],
  };

  const incomingByKey = new Map(
    incoming.flatMap((event) => {
      const calendarId = ctx.calendarTrackingIdToId.get(
        event.tracking_id_calendar,
      );
      return calendarId
        ? [[eventKey(calendarId, event.tracking_id_event), event] as const]
        : [];
    }),
  );
  const handledKeys = new Set<string>();

  for (const storeEvent of existing) {
    const trackingId = storeEvent.tracking_id_event;
    const key = eventKey(storeEvent.calendar_id, trackingId);
    const matchingIncomingEvent = incomingByKey.get(key);

    if (matchingIncomingEvent && !handledKeys.has(key)) {
      out.toUpdate.push({
        ...storeEvent,
        ...matchingIncomingEvent,
        id: storeEvent.id,
        tracking_id_event: trackingId,
        created_at: storeEvent.created_at,
        calendar_id: storeEvent.calendar_id,
        has_recurrence_rules: matchingIncomingEvent.has_recurrence_rules,
        participants: incomingParticipants.get(trackingId) ?? [],
      });
      handledKeys.add(key);
      continue;
    }

    if (!storeEvent.deleted_at) {
      out.toDelete.push(storeEvent.id);
    }
  }

  const scheduledKeys = new Set(handledKeys);
  for (const incomingEvent of incoming) {
    const calendarId = ctx.calendarTrackingIdToId.get(
      incomingEvent.tracking_id_calendar,
    );
    const key = calendarId
      ? eventKey(calendarId, incomingEvent.tracking_id_event)
      : null;
    if (!key || !scheduledKeys.has(key)) {
      out.toAdd.push({
        ...incomingEvent,
        participants:
          incomingParticipants.get(incomingEvent.tracking_id_event) ?? [],
      });
      if (key) scheduledKeys.add(key);
    }
  }

  return out;
}

function eventKey(calendarId: string, trackingId: string): string {
  return `${calendarId}\u0000${trackingId}`;
}
