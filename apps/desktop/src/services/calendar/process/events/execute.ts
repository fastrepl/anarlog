import type { SessionEvent } from "@hypr/store";

import type { Ctx } from "../../ctx";
import type { IncomingEvent } from "../../fetch/types";
import type { EventsSyncOutput } from "./types";

import { deleteEvent, insertEvent, updateEvent } from "~/calendar/queries";
import { getSessionEventById } from "~/session/utils";
import { id } from "~/shared/utils";

export async function executeForEventsSync(
  ctx: Ctx,
  out: EventsSyncOutput,
): Promise<void> {
  const userId = ctx.userId;
  if (!userId) {
    throw new Error("user_id is not set");
  }

  const now = new Date().toISOString();

  for (const eventId of out.toDelete) {
    await deleteEvent(eventId);
  }

  for (const event of out.toUpdate) {
    await updateEvent(event.id, {
      trackingIdEvent: event.tracking_id_event,
      calendarId: event.calendar_id,
      title: event.title,
      startedAt: event.started_at,
      endedAt: event.ended_at,
      location: event.location,
      meetingLink: event.meeting_link,
      description: event.description,
      recurrenceSeriesId: event.recurrence_series_id,
      hasRecurrenceRules: event.has_recurrence_rules,
      isAllDay: event.is_all_day,
      provider: ctx.provider,
      participantsJson:
        event.participants.length > 0
          ? JSON.stringify(event.participants)
          : undefined,
    });
  }

  for (const eventToAdd of out.toAdd) {
    const calendarId = ctx.calendarTrackingIdToId.get(
      eventToAdd.tracking_id_calendar,
    );
    if (!calendarId) {
      continue;
    }

    await insertEvent({
      id: id(),
      trackingIdEvent: eventToAdd.tracking_id_event,
      calendarId,
      title: eventToAdd.title ?? "",
      startedAt: eventToAdd.started_at ?? "",
      endedAt: eventToAdd.ended_at ?? "",
      location: eventToAdd.location ?? "",
      meetingLink: eventToAdd.meeting_link ?? "",
      description: eventToAdd.description ?? "",
      recurrenceSeriesId: eventToAdd.recurrence_series_id ?? "",
      hasRecurrenceRules: eventToAdd.has_recurrence_rules,
      isAllDay: eventToAdd.is_all_day,
      provider: ctx.provider,
      participantsJson:
        eventToAdd.participants.length > 0
          ? JSON.stringify(eventToAdd.participants)
          : undefined,
      createdAt: now,
    });
  }
}

export function syncSessionEmbeddedEvents(
  ctx: Ctx,
  incoming: IncomingEvent[],
): void {
  const incomingByTrackingId = new Map<string, IncomingEvent>();
  for (const event of incoming) {
    incomingByTrackingId.set(event.tracking_id_event, event);
  }

  ctx.store.transaction(() => {
    ctx.store.forEachRow("sessions", (sessionId, _forEachCell) => {
      const sessionEvent = getSessionEventById(ctx.store, sessionId);
      if (!sessionEvent) return;

      const incomingEvent = incomingByTrackingId.get(sessionEvent.tracking_id);
      if (!incomingEvent) return;

      const calendarId =
        ctx.calendarTrackingIdToId.get(incomingEvent.tracking_id_calendar) ??
        "";

      const updated: SessionEvent = {
        tracking_id: incomingEvent.tracking_id_event,
        calendar_id: calendarId,
        title: incomingEvent.title ?? "",
        started_at: incomingEvent.started_at ?? "",
        ended_at: incomingEvent.ended_at ?? "",
        is_all_day: incomingEvent.is_all_day,
        has_recurrence_rules: incomingEvent.has_recurrence_rules,
        location: incomingEvent.location,
        meeting_link: incomingEvent.meeting_link,
        description: incomingEvent.description,
        recurrence_series_id: incomingEvent.recurrence_series_id,
      };

      ctx.store.setPartialRow("sessions", sessionId, {
        event_json: JSON.stringify(updated),
      });
    });
  });
}
