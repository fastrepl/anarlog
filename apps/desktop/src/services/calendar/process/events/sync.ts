import type { Ctx } from "../../ctx";
import {
  buildIncomingEventIndex,
  calendarEventKey,
  calendarEventKeys,
} from "./identity";
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

  const { canonical: incomingByCanonicalKey, expanded: incomingByKey } =
    buildIncomingEventIndex(ctx.provider, incoming, ctx.calendarTrackingIdToId);
  const incomingCalendarsByTrackingId = new Map<string, Set<string>>();
  for (const event of incoming) {
    const calendarId = ctx.calendarTrackingIdToId.get(
      event.tracking_id_calendar,
    );
    if (!calendarId) continue;
    const calendarIds =
      incomingCalendarsByTrackingId.get(event.tracking_id_event) ??
      new Set<string>();
    calendarIds.add(calendarId);
    incomingCalendarsByTrackingId.set(event.tracking_id_event, calendarIds);
  }
  const handledKeys = new Set<string>();

  for (const storeEvent of existing) {
    const matchingIncomingEvent = calendarEventKeys(
      ctx.provider,
      storeEvent.calendar_id,
      storeEvent,
    )
      .map((key) => incomingByKey.get(key))
      .find((event) => event !== undefined);
    const key = matchingIncomingEvent
      ? calendarEventKey(
          ctx.provider,
          storeEvent.calendar_id,
          matchingIncomingEvent,
        )
      : calendarEventKey(ctx.provider, storeEvent.calendar_id, storeEvent);

    if (matchingIncomingEvent && !handledKeys.has(key)) {
      out.toUpdate.push({
        ...storeEvent,
        ...matchingIncomingEvent,
        id: storeEvent.id,
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

    const incomingCalendars = incomingCalendarsByTrackingId.get(
      storeEvent.tracking_id_event,
    );
    const movedToAnotherCalendar =
      incomingCalendars !== undefined &&
      !incomingCalendars.has(storeEvent.calendar_id);
    if (
      !storeEvent.deleted_at &&
      (movedToAnotherCalendar ||
        overlapsSyncRange(ctx, storeEvent.started_at, storeEvent.ended_at))
    ) {
      out.toDelete.push(storeEvent.id);
    }
  }

  const scheduledKeys = new Set(handledKeys);
  for (const [key, incomingEvent] of incomingByCanonicalKey) {
    if (!scheduledKeys.has(key)) {
      out.toAdd.push({
        ...incomingEvent,
        participants:
          incomingParticipants.get(incomingEvent.tracking_id_event) ?? [],
      });
      scheduledKeys.add(key);
    }
  }

  return out;
}

function overlapsSyncRange(
  ctx: Ctx,
  startedAt: string,
  endedAt: string,
): boolean {
  const start = Date.parse(startedAt);
  const end = Date.parse(endedAt || startedAt);
  if (!Number.isFinite(start) || !Number.isFinite(end)) return true;

  return start <= ctx.to.getTime() && end >= ctx.from.getTime();
}
