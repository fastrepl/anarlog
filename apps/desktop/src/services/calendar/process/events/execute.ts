import type { SessionEvent } from "@hypr/store";

import type { Ctx } from "../../ctx";
import type { IncomingEvent } from "../../fetch/types";
import type { SessionSyncRow } from "../../storage";

export type SessionEventUpdate = {
  sessionId: string;
  trackingId: string;
  calendarId: string;
  seriesId: string;
  eventJson: string;
};

export function syncSessionEmbeddedEvents(
  ctx: Ctx,
  incoming: IncomingEvent[],
  sessions: SessionSyncRow[],
): SessionEventUpdate[] {
  const incomingByTrackingId = new Map(
    incoming.map((event) => [event.tracking_id_event, event]),
  );
  const updates: SessionEventUpdate[] = [];

  for (const session of sessions) {
    const incomingEvent = incomingByTrackingId.get(session.trackingId);
    if (!incomingEvent) continue;

    const calendarId =
      ctx.calendarTrackingIdToId.get(incomingEvent.tracking_id_calendar) ?? "";
    const event: SessionEvent = {
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

    updates.push({
      sessionId: session.id,
      trackingId: incomingEvent.tracking_id_event,
      calendarId,
      seriesId: incomingEvent.recurrence_series_id ?? "",
      eventJson: JSON.stringify(event),
    });
  }

  return updates;
}
