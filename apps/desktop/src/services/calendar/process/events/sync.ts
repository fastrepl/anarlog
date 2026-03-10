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

  const incomingByTrackingId = new Map(
    incoming.map((e) => [e.tracking_id_event, e]),
  );
  const handledTrackingIds = new Set<string>();

  for (const storeEvent of existing) {
    if (!ctx.calendarIds.has(storeEvent.calendar_id!)) {
      out.toDelete.push(storeEvent.id);
      continue;
    }

    const trackingId = storeEvent.tracking_id_event;
    const matchingIncomingEvent = trackingId
      ? incomingByTrackingId.get(trackingId)
      : undefined;

    if (matchingIncomingEvent && trackingId) {
      out.toUpdate.push({
        ...storeEvent,
        ...matchingIncomingEvent,
        id: storeEvent.id,
        tracking_id_event: trackingId,
        user_id: storeEvent.user_id,
        created_at: storeEvent.created_at,
        calendar_id: storeEvent.calendar_id,
        has_recurrence_rules: matchingIncomingEvent.has_recurrence_rules,
        participants: incomingParticipants.get(trackingId) ?? [],
      });
      handledTrackingIds.add(trackingId);
      continue;
    }

    out.toDelete.push(storeEvent.id);
  }

  for (const incomingEvent of incoming) {
    if (!handledTrackingIds.has(incomingEvent.tracking_id_event)) {
      out.toAdd.push({
        ...incomingEvent,
        participants:
          incomingParticipants.get(incomingEvent.tracking_id_event) ?? [],
      });
    }
  }

  return out;
}
