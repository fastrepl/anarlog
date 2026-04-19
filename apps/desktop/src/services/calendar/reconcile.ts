import type { CalendarProviderType } from "@hypr/plugin-calendar";
import type { EventParticipant } from "@hypr/store";

import type { Ctx } from "./ctx";
import type { IncomingEvent, IncomingParticipants } from "./fetch/types";
import {
  executeForParticipantsSync,
  syncSessionParticipants,
} from "./process/participants";

import { getSessionEventById } from "~/session/utils";
import type { Store } from "~/store/tinybase/store/main";

const DEFAULT_PROVIDER = "apple" as CalendarProviderType;
type ReconcileIncomingEvent = IncomingEvent & { calendar_id: string };

export function reconcileCalendarSessions(store: Store) {
  const ctx = createReconcileCtx(store);
  const incoming: ReconcileIncomingEvent[] = [];
  const incomingParticipants: IncomingParticipants = new Map();

  store.forEachRow("events", (eventId, _forEachCell) => {
    const event = store.getRow("events", eventId);
    if (!event?.tracking_id_event) {
      return;
    }

    const calendarId = String(event.calendar_id ?? "");
    incoming.push({
      tracking_id_event: String(event.tracking_id_event),
      tracking_id_calendar: calendarId,
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

    const participants = parseParticipants(event.participants_json);
    if (participants.length > 0) {
      incomingParticipants.set(String(event.tracking_id_event), participants);
    }
  });

  reconcileSessionEmbeddedEvents(store, incoming);

  const participantsOut = syncSessionParticipants(ctx, {
    incomingParticipants,
  });
  executeForParticipantsSync(ctx, participantsOut);
}

function createReconcileCtx(store: Store): Ctx {
  const calendarTrackingIdToId = new Map<string, string>();
  store.forEachRow("calendars", (calendarId, _forEachCell) => {
    const calendar = store.getRow("calendars", calendarId);
    const trackingId = calendar?.tracking_id_calendar;
    if (typeof trackingId === "string" && trackingId) {
      calendarTrackingIdToId.set(trackingId, calendarId);
    }
  });

  return {
    store,
    provider: DEFAULT_PROVIDER,
    connectionId: "",
    userId: String(store.getValue("user_id") ?? ""),
    from: new Date(0),
    to: new Date(0),
    calendarIds: new Set(),
    calendarTrackingIdToId,
  };
}

function reconcileSessionEmbeddedEvents(
  store: Store,
  incoming: ReconcileIncomingEvent[],
) {
  const incomingByTrackingId = new Map<string, ReconcileIncomingEvent>();
  for (const event of incoming) {
    incomingByTrackingId.set(event.tracking_id_event, event);
  }

  store.transaction(() => {
    store.forEachRow("sessions", (sessionId, _forEachCell) => {
      const sessionEvent = getSessionEventById(store, sessionId);
      if (!sessionEvent) return;

      const incomingEvent = incomingByTrackingId.get(sessionEvent.tracking_id);
      if (!incomingEvent) return;

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
