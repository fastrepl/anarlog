import type { EventStorage, SessionEvent } from "@hypr/store";

import type { Ctx } from "../../ctx";
import type { IncomingEvent } from "../../fetch/types";
import type { EventsSyncOutput } from "./types";

import { getSessionEventById } from "~/session/utils";
import { id } from "~/shared/utils";

export function executeForEventsSync(ctx: Ctx, out: EventsSyncOutput): void {
  const userId = ctx.store.getValue("user_id");
  if (!userId) {
    throw new Error("user_id is not set");
  }

  const now = new Date().toISOString();

  ctx.store.transaction(() => {
    for (const eventId of out.toDelete) {
      ctx.store.delRow("events", eventId);
    }

    for (const event of out.toUpdate) {
      ctx.store.setPartialRow("events", event.id, {
        tracking_id_event: event.tracking_id_event,
        calendar_id: event.calendar_id,
        title: event.title,
        started_at: event.started_at,
        ended_at: event.ended_at,
        location: event.location,
        meeting_link: event.meeting_link,
        description: event.description,
        recurrence_series_id: event.recurrence_series_id,
        has_recurrence_rules: event.has_recurrence_rules,
        is_all_day: event.is_all_day,
        provider: ctx.provider,
        participants_json:
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

      const eventId = id();

      ctx.store.setRow("events", eventId, {
        user_id: userId,
        created_at: now,
        tracking_id_event: eventToAdd.tracking_id_event,
        calendar_id: calendarId,
        title: eventToAdd.title ?? "",
        started_at: eventToAdd.started_at ?? "",
        ended_at: eventToAdd.ended_at ?? "",
        location: eventToAdd.location,
        meeting_link: eventToAdd.meeting_link,
        description: eventToAdd.description,
        recurrence_series_id: eventToAdd.recurrence_series_id,
        has_recurrence_rules: eventToAdd.has_recurrence_rules,
        is_all_day: eventToAdd.is_all_day,
        provider: ctx.provider,
        participants_json:
          eventToAdd.participants.length > 0
            ? JSON.stringify(eventToAdd.participants)
            : undefined,
      } satisfies EventStorage);
    }
  });
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
