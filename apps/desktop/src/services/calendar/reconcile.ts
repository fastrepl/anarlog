import type { EventParticipant } from "@hypr/store";

import {
  executeForParticipantsSync,
  syncSessionParticipants,
} from "./process/participants";
import type {
  IncomingParticipants,
  ReconcileCtx,
  ReconcileIncomingEvent,
} from "./types";

import { getSessionEventById } from "~/session/utils";
import type { Store } from "~/store/tinybase/store/main";

export function reconcileCalendarSessions(store: Store) {
  const ctx: ReconcileCtx = { store };
  const incomingByTrackingId = new Map<string, ReconcileIncomingEvent>();
  const incomingParticipants: IncomingParticipants = new Map();

  store.forEachRow("events", (eventId, _forEachCell) => {
    const event = store.getRow("events", eventId);
    if (!event?.tracking_id_event) {
      return;
    }

    const trackingId = String(event.tracking_id_event);
    const calendarId = String(event.calendar_id ?? "");
    incomingByTrackingId.set(trackingId, {
      tracking_id_event: trackingId,
      calendar_id: calendarId,
      title: asOptionalString(event.title),
      started_at: asOptionalString(event.started_at),
      ended_at: asOptionalString(event.ended_at),
      location: asOptionalString(event.location),
      meeting_link: asOptionalString(event.meeting_link),
      description: asOptionalString(event.description),
      recurrence_series_id: asOptionalString(event.recurrence_series_id),
      has_recurrence_rules: Boolean(event.has_recurrence_rules),
      is_all_day: Boolean(event.is_all_day),
    });

    incomingParticipants.set(trackingId, {
      type: "observed",
      participants: parseParticipants(event.participants_json),
    });
  });

  reconcileSessionEmbeddedEvents(
    store,
    incomingByTrackingId,
    incomingParticipants,
  );

  const participantsOut = syncSessionParticipants(ctx, {
    incomingParticipants,
  });
  executeForParticipantsSync(ctx, participantsOut);
}

function reconcileSessionEmbeddedEvents(
  store: Store,
  incomingByTrackingId: Map<string, ReconcileIncomingEvent>,
  incomingParticipants: IncomingParticipants,
) {
  store.transaction(() => {
    store.forEachRow("sessions", (sessionId, _forEachCell) => {
      const sessionEvent = getSessionEventById(store, sessionId);
      if (!sessionEvent) return;
      if (!sessionEvent.tracking_id) return;

      const incomingEvent = incomingByTrackingId.get(sessionEvent.tracking_id);
      if (!incomingEvent) {
        // During the TinyBase bridge, calendar-sync keeps out-of-range events
        // in the cache, so a missing row is a positive delete.
        store.setPartialRow("sessions", sessionId, {
          event_json: "",
        });
        incomingParticipants.set(sessionEvent.tracking_id, { type: "deleted" });
        return;
      }

      store.setPartialRow("sessions", sessionId, {
        event_json: JSON.stringify({
          tracking_id: incomingEvent.tracking_id_event,
          calendar_id: incomingEvent.calendar_id,
          title: incomingEvent.title ?? "",
          started_at: incomingEvent.started_at ?? "",
          ended_at: incomingEvent.ended_at ?? "",
          is_all_day: incomingEvent.is_all_day,
          has_recurrence_rules: incomingEvent.has_recurrence_rules,
          location: incomingEvent.location,
          meeting_link: incomingEvent.meeting_link,
          description: incomingEvent.description,
          recurrence_series_id: incomingEvent.recurrence_series_id,
        }),
      });
    });
  });
}

function asOptionalString(value: unknown) {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function parseParticipants(value: unknown): EventParticipant[] {
  if (typeof value !== "string" || !value) {
    return [];
  }

  try {
    const parsed = JSON.parse(value) as EventParticipant[];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}
