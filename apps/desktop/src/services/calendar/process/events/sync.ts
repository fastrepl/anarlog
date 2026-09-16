import type { Ctx } from "../../ctx";
import { eventKey, indexIncomingEvents } from "./identity";
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

  const incomingByKey = indexIncomingEvents(ctx, incoming);
  const handledKeys = new Set<string>();

  for (const storeEvent of existing) {
    const trackingId = storeEvent.tracking_id_event;
    const matchingIncomingEvent = incomingByKey.get(
      eventKey(storeEvent.calendar_id, trackingId),
    );
    const key = eventKey(
      storeEvent.calendar_id,
      matchingIncomingEvent?.tracking_id_event ?? trackingId,
    );

    if (
      matchingIncomingEvent &&
      !matchingIncomingEvent.is_cancelled &&
      !handledKeys.has(key)
    ) {
      out.toUpdate.push({
        ...storeEvent,
        ...matchingIncomingEvent,
        id: storeEvent.id,
        tracking_id_event: matchingIncomingEvent.tracking_id_event,
        created_at: storeEvent.created_at,
        calendar_id: storeEvent.calendar_id,
        has_recurrence_rules: matchingIncomingEvent.has_recurrence_rules,
        participants:
          incomingParticipants.get(matchingIncomingEvent.tracking_id_event) ??
          [],
      });
      handledKeys.add(key);
      continue;
    }

    const overlapsRange =
      Date.parse(storeEvent.started_at) <= ctx.to.getTime() &&
      Date.parse(storeEvent.ended_at || storeEvent.started_at) >=
        ctx.from.getTime();
    if (!storeEvent.deleted_at && (matchingIncomingEvent || overlapsRange)) {
      out.toDelete.push(storeEvent.id);
    }
  }

  const scheduledKeys = new Set(handledKeys);
  for (const incomingEvent of incoming) {
    if (incomingEvent.is_cancelled) continue;
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
