import type { Ctx } from "../ctx";
import type { ExistingEvent } from "./types";

function isEventInRange(
  startedAt: string,
  endedAt: string | undefined,
  from: Date,
  to: Date,
): boolean {
  const eventStart = new Date(startedAt);
  const eventEnd = endedAt ? new Date(endedAt) : eventStart;

  return eventStart <= to && eventEnd >= from;
}

export function fetchExistingEvents(ctx: Ctx): ExistingEvent[] {
  const events: ExistingEvent[] = [];

  ctx.store.forEachRow("events", (rowId, _forEachCell) => {
    const event = ctx.store.getRow("events", rowId);
    if (!event) return;

    const calendarId = event.calendar_id;
    if (!calendarId) {
      return;
    }

    if (event.provider && event.provider !== ctx.provider) {
      return;
    }

    const startedAt = event.started_at;
    if (!startedAt) return;

    const endedAt = event.ended_at;
    if (isEventInRange(startedAt, endedAt, ctx.from, ctx.to)) {
      events.push({
        id: rowId,
        tracking_id_event: event.tracking_id_event,
        user_id: event.user_id,
        created_at: event.created_at,
        calendar_id: calendarId,
        title: event.title,
        started_at: startedAt,
        ended_at: endedAt,
        location: event.location,
        meeting_link: event.meeting_link,
        description: event.description,
        note: event.note,
        recurrence_series_id: event.recurrence_series_id,
        has_recurrence_rules: event.has_recurrence_rules,
        provider: event.provider,
      });
    }
  });

  return events;
}
