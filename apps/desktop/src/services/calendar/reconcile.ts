import type { CalendarProviderType } from "@hypr/plugin-calendar";
import type { EventParticipant } from "@hypr/store";

import type { Ctx } from "./ctx";
import type { IncomingEvent, IncomingParticipants } from "./fetch/types";
import { syncSessionEmbeddedEvents } from "./process/events";
import {
  executeForParticipantsSync,
  syncSessionParticipants,
} from "./process/participants";

import type { Store } from "~/store/tinybase/store/main";

const DEFAULT_PROVIDER = "apple" as CalendarProviderType;

export function reconcileCalendarSessions(store: Store) {
  const ctx = createReconcileCtx(store);
  const incoming: IncomingEvent[] = [];
  const incomingParticipants: IncomingParticipants = new Map();

  store.forEachRow("events", (eventId, _forEachCell) => {
    const event = store.getRow("events", eventId);
    if (!event?.tracking_id_event) {
      return;
    }

    const calendarId = String(event.calendar_id ?? "");
    if (calendarId) {
      ctx.calendarTrackingIdToId.set(calendarId, calendarId);
    }

    incoming.push({
      tracking_id_event: String(event.tracking_id_event),
      tracking_id_calendar: calendarId,
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

  syncSessionEmbeddedEvents(ctx, incoming);

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
